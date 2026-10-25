// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MarkdownToolbar` — the formatting strip paired with [`MarkdownEditor`](super::MarkdownEditor).
//!
//! The editor publishes [`MarkdownEditor::wrap_selection`](super::MarkdownEditor::wrap_selection) and
//! [`MarkdownEditor::toggle_line_prefix`](super::MarkdownEditor::toggle_line_prefix) as its single
//! formatting entry points (issue COMP-H04 /
//! SP-03). This control is the pointer half of that contract: it lays the enabled actions out as
//! one row of cells and emits the token of the cell a reader activates. It deliberately does **not**
//! hold the editor — a toolbar that reached into a text control would be the second implementation
//! path for the same edit, and the host already knows both handles.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_string_list};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which side of a cell's label the icon token is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolbarIconSide {
    /// No icon, label only.
    None,
    /// The icon precedes the label.
    #[default]
    Left,
    /// The icon follows the label.
    Right,
}

impl ToolbarIconSide {
    /// The published token spelling.
    pub fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Parses the published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "none" => Some(Self::None),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// The actions a fresh toolbar offers, in the order they are painted.
///
/// Every token is also the payload of [`MarkdownToolbar::action_triggered`], so a host can match on
/// it directly.
pub const MARKDOWN_TOOLBAR_ACTIONS: [&str; 7] =
    ["bold", "italic", "code", "link", "heading", "list", "blockquote"];

/// The short glyph drawn in a cell's icon slot.
///
/// The bundled faces do not all cover typographic symbols, so the icon slot uses ASCII letters
/// rather than a glyph that would fall back to an 8×8 bitmap.
const ACTION_ICONS: [(&str, &str); 7] = [
    ("bold", "B"),
    ("italic", "I"),
    ("code", "C"),
    ("link", "L"),
    ("heading", "H"),
    ("list", "U"),
    ("blockquote", "Q"),
];

/// One cell in the toolbar's row.
const CELL_HEIGHT: i32 = 22;
/// Minimum cell width, so a narrow toolbar still has tappable cells.
const MIN_CELL_WIDTH: i32 = 26;

/// A horizontal formatting strip for a markdown editor.
///
/// The control owns only the action list and its presentation. Activating a cell emits
/// [`Self::action_triggered`]; the host forwards that token to the editor's own action
/// ([`MarkdownEditor::wrap_selection`](super::MarkdownEditor::wrap_selection) or
/// [`MarkdownEditor::toggle_line_prefix`](super::MarkdownEditor::toggle_line_prefix)).
pub struct MarkdownToolbar {
    base: BaseWidget,
    actions: Vec<String>,
    icon_side: ToolbarIconSide,
    show_labels: bool,
    hovered: Option<usize>,
    pressed: Option<usize>,
    /// Emitted with the token of the action a reader activated.
    pub action_triggered: Signal1<String>,
}

impl MarkdownToolbar {
    /// Creates a toolbar with the default action set.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "MarkdownToolbar"),
            actions: MARKDOWN_TOOLBAR_ACTIONS.iter().map(|token| (*token).to_string()).collect(),
            icon_side: ToolbarIconSide::Left,
            show_labels: false,
            hovered: None,
            pressed: None,
            action_triggered: Signal1::new(),
        }
    }

    /// The enabled actions, in paint order.
    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    /// Replaces the enabled actions.
    ///
    /// Unknown tokens are refused rather than stored: an action the control cannot execute would
    /// paint a cell that does nothing, which is the silent failure this crate's published-action
    /// rule exists to prevent.
    pub fn set_actions(&mut self, actions: Vec<String>) -> bool {
        if actions.iter().any(|token| action_icon(token).is_none()) {
            return false;
        }
        if self.actions == actions {
            return true;
        }
        self.actions = actions;
        self.hovered = None;
        self.pressed = None;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Which side of a label the icon is drawn on.
    pub fn icon_side(&self) -> ToolbarIconSide {
        self.icon_side
    }

    /// Sets which side of a label the icon is drawn on.
    pub fn set_icon_side(&mut self, side: ToolbarIconSide) {
        if self.icon_side == side {
            return;
        }
        self.icon_side = side;
        self.base.request_redraw();
    }

    /// Whether the text label is painted beside the icon.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Shows or hides the text labels.
    pub fn set_show_labels(&mut self, show: bool) {
        if self.show_labels == show {
            return;
        }
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Runs `action_token`, emitting it when it is one of the enabled actions.
    ///
    /// Returns whether the token resolved. This is the same path a cell press takes, so a host
    /// menu item and the toolbar cell cannot report different actions.
    pub fn execute(&mut self, action_token: &str) -> bool {
        if !self.actions.iter().any(|token| token == action_token) {
            return false;
        }
        self.action_triggered.emit(action_token.to_string());
        true
    }

    /// The box of the cell at `index`.
    fn cell_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let rect = self.geometry();
        let count = self.actions.len().max(1) as i32;
        let width = (rect.width as i32 / count).max(MIN_CELL_WIDTH);
        let top = rect.y + (rect.height as i32 - CELL_HEIGHT).max(0) / 2;
        Some(Rect::new(rect.x + index as i32 * width, top, width as u32, CELL_HEIGHT as u32))
    }

    /// The cell under `pos`.
    fn cell_at(&self, pos: Point) -> Option<usize> {
        (0..self.actions.len())
            .find(|index| self.cell_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }
}

/// The short glyph a published action token is drawn with.
fn action_icon(token: &str) -> Option<&'static str> {
    ACTION_ICONS.iter().find(|(name, _)| *name == token).map(|(_, icon)| *icon)
}

impl Widget for MarkdownToolbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        Size::new(self.actions.len().max(1) as u32 * 34, 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for MarkdownToolbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "actions" => Ok(CapabilityValue::Tuple(
                self.actions.iter().cloned().map(CapabilityValue::String).collect(),
            )),
            "icon_side" => Ok(CapabilityValue::String(self.icon_side.token().to_string())),
            "show_labels" => Ok(CapabilityValue::Bool(self.show_labels)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "actions" => {
                let actions = expect_string_list(value)?;
                if !self.set_actions(actions) {
                    return Err(CapabilityAccessError::TypeMismatch);
                }
                Ok(())
            }
            "icon_side" => {
                let token = expect_string(value)?;
                let side = ToolbarIconSide::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_icon_side(side);
                Ok(())
            }
            "show_labels" => {
                self.set_show_labels(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["actions", "icon_side", "show_labels", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for MarkdownToolbar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let next = self.cell_at(*pos);
                if next != self.hovered {
                    self.hovered = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered.take().is_some() || self.pressed.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.cell_at(*pos) {
                    self.pressed = Some(index);
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                let pressed = self.pressed.take();
                self.base.request_redraw();
                if let Some(index) = pressed {
                    if self.cell_at(*pos) == Some(index) {
                        if let Some(token) = self.actions.get(index).cloned() {
                            self.execute(&token);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Draw for MarkdownToolbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("tool_bar");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| Color::border_of(surface));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = effective_font(style);

        context.face_with_gradient(
            rect,
            surface,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().border_radius.unwrap_or(0),
            border,
            1,
        );

        for index in 0..self.actions.len() {
            let Some(cell) = self.cell_rect(index) else {
                continue;
            };
            let token = self.actions[index].as_str();
            let pressed = self.pressed == Some(index);
            if pressed {
                context.fill_rounded_rect(cell, 3, accent.with_alpha(70));
            } else if self.hovered == Some(index) {
                context.fill_rounded_rect(cell, 3, surface.blend(&ink, 0.08));
            }
            let icon = action_icon(token).unwrap_or("");
            let label = if self.show_labels { token } else { "" };
            let text = match (self.icon_side, label.is_empty()) {
                (_, true) => icon.to_string(),
                (ToolbarIconSide::None, false) => label.to_string(),
                (ToolbarIconSide::Left, false) => format!("{icon} {label}"),
                (ToolbarIconSide::Right, false) => format!("{label} {icon}"),
            };
            context.draw_text_fitted_cow(
                cell,
                &text,
                font,
                if self.hovered == Some(index) { ink } else { ink.blend(&surface, 0.15) },
                HorizontalAlignment::Center,
            );
        }
    }
}
