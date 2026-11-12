// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Button widget implementation.
#[cfg(full_widgets)]
use crate::compat::Vec;
use crate::compat::{format, RefCell, String, ToString};
#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
#[cfg(full_widgets)]
use crate::event::key_codes;
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::base_widgets::button_group::{self, GroupKey};
use crate::widget::base_widgets::label::{draw_elided_line, TextElideMode};
use crate::widget::base_widgets::AutoRepeat;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::menu_button::{
    draw_menu_panel, menu_item_rect, menu_panel_rect, MenuItem,
};
use crate::widget::metrics::{
    active_profile, dimensions, effective_font, estimate_line_height, estimate_text_width,
    focus_ring_color, profile_metrics, ControlMetrics, ControlMetricsProfile, FocusRing,
    FOCUS_HALO_BLUR_RADIUS, FOCUS_RING_WIDTH,
};
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, IconSource, IconTint, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

mod api;
mod paint;
mod state;

pub use state::{Button, ButtonState, IconPosition};
pub(crate) use state::{ButtonHintKey, DEFAULT_EMPHASIS_WIDTH};

#[cfg(test)]
mod tests;
