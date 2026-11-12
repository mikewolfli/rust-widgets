// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! JSON layout loader — parses JSON source and instantiates widget trees.
//!
//! The [`JsonLoader`] reads a JSON string, recursively creates
//! widget instances, applies properties (geometry, style, events),
//! and returns a [`BoundJsonLayout`](crate::json::BoundJsonLayout)
//! for typed widget access.
//!
//! # i18n Support
//!
//! Text properties (`title`, `text`, `tooltip`, `placeholder`) accept
//! translation keys. Use the [`crate::i18n::I18nManager::translate`] or
//! the [`crate::tr!`] macro at the call site to resolve the key before
//! passing JSON to the loader. The loader itself does NOT call the
//! i18n manager – it uses the literal string values from JSON.
//!
//! # Event Binding
//!
//! When a JSON node declares `"on_click": "handler_name"`, the string
//! is stored in the loader and connected after instantiation.
//! Callers use [`EventHandlerMap`](crate::json::EventHandlerMap) to
//! register closures against those handler names.

use std::borrow::Cow;

use serde_json::Value;

use crate::app::{ButtonHandle, WidgetHandle};
use crate::core::{Color, ObjectId, Rect};
use crate::json::properties::ApplyOutcome;
use crate::json::{
    add_spacer_to_layout, add_widget_to_layout, apply_layout, create_layout_from_kind,
    parse_layout_kind, store_layout, BoundJsonLayout, ChildLayoutAttrs,
};
use crate::layout::inspector::LayoutInspector;
use crate::style::ElementRef;
use crate::widget::{
    Button, CheckBox, ComboBox, GroupBox, Label, LineEdit, ListBox, ProgressBar, RadioButton,
    ScrollArea, ScrollBar, Slider, SpinBox, Switch, Widget,
};
// Container / menu content routing (issue-20261104 DM-06/07/08/11/12). These are the
// concrete types whose own content methods `route_container_content` calls; the list lives
// here rather than in each arm so the routing table reads as data.
use crate::widget::container_widgets::linear_layouts::{
    Flex, FlexItemSpec, FlowLayout, WrapLayout,
};
use crate::widget::container_widgets::Accordion;
use crate::widget::container_widgets::Carousel;
use crate::widget::container_widgets::MasonryLayout;
use crate::widget::dialog::wizard::WizardDialog;
use crate::widget::menu_toolbar::action::Action;
use crate::widget::menu_toolbar::menu::Menu;
use crate::widget::{
    AdaptiveScaffold, BottomSheet, CollapsiblePane, Dialog, DockWidget, Frame, MdiArea, MenuBar,
    ModalBottomSheet, NavigationStack, PopupWindow, SafeArea, Splitter, StackedWidget, StatusBar,
    TabView, ToolBar, ToolBox,
};
#[cfg(not(alloc_frugal))]
use crate::widget::{
    ColorDialog, FileDialog, FontDialog, GridWidget, ListView, MessageBox, TabWidget, TextEdit,
};
use crate::window::Window;
// `WidgetKind` is named only by the test-only `infer_kind` table below; production
// registration reads the live control's kind instead.
#[cfg(test)]
use crate::index::WidgetKind;

mod nodes;
mod wiring;

pub use nodes::JsonLoader;
#[cfg(test)]
pub(crate) use nodes::DEFAULT_DECLARED_CARD_HEIGHT;
pub(crate) use wiring::apply_declared_styles;
#[cfg(test)]
pub(crate) use wiring::apply_properties;
pub(crate) use wiring::apply_properties_with_icon_resolver;
pub(crate) use wiring::apply_range_like;
pub use wiring::bind_event_binding;
pub(crate) use wiring::breakpoints_include;
pub(crate) use wiring::for_each_json_item;
#[cfg(test)]
pub(crate) use wiring::infer_kind;
pub use wiring::is_loader_owned_key;
pub(crate) use wiring::json_geometry;
#[cfg(test)]
pub(crate) use wiring::json_orientation;
pub(crate) use wiring::json_u32_at;
pub(crate) use wiring::json_usize_at;
pub use wiring::load_layout_from_str;
pub(crate) use wiring::lowered;
pub(crate) use wiring::mount_declared_widget;
pub(crate) use wiring::parse_declared_id;
#[cfg(test)]
pub(crate) use wiring::read_json_range;
#[cfg(test)]
pub(crate) use wiring::MARKER_KEYS;

#[cfg(test)]
mod tests;
