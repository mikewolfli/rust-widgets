// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MiniChart widget — simplified line/bar chart for mini builds (BLUE13 R2.10).

use core::fmt::Write as _;

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
// The cartesian-engine widgets draw their axes from one shared derivation in
// `chart_widgets::charts`. That module is **absent on the `embedded` profile**, where
// `mini_chart` still exists, so the derivation cannot be imported here: doing so broke the
// `embedded` build outright. The rule the shared helper encodes is a single blend along the
// surface-to-ink axis, which is three lines and needs nothing but the active theme, so this
// control derives its own from the *same resolved pair* its surface uses. What matters for
// the defect being fixed is that the grid follows the appearance and that the grid is fainter
// than the axis — not which module the arithmetic lives in.
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The two styles a [`MiniChart`] can draw: a connected line or vertical bars.
///
/// # Scope, and why this is not the same enum as the other `ChartType`s
///
/// A `MiniChart` is defined by having no axes, grid or legend, so it only ever
/// needs these two styles. Three same-named enums exist across the chart layers,
/// each scoped to its own widget — kept separate deliberately, so a `match` in
/// one widget is exhaustive:
///
/// * **this one** — [`MiniChart`]: `Line`, `Bar`;
/// * `crate::widget::special_widgets::chart::ChartType` — `ChartWidget`:
///   `Bar`, `Line`, `Pie`, `Scatter`;
/// * `crate::widget::chart_widgets::types::ChartType` — the drawing engine:
///   `Line`, `Bar`, `Pie`, `Scatter`, `Area`.
///
/// See principle #49.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    /// Connected line chart.
    Line,
    /// Vertical bar chart.
    Bar,
}

/// A single data point with a label and numeric value.
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// Data point label.
    pub label: String,
    /// Numeric value.
    pub value: u32,
}

impl DataPoint {
    /// Creates a new data point.
    ///
    /// The label accepts either text or a numeric index. Text is the normal form; an integer is
    /// the categorical-index spelling a caller uses when the point is identified by position
    /// ("0", "1", …), and both arrive as the same `String` label. Keeping one constructor for the
    /// two spellings avoids a second `from_index` entry point that would have to be kept in step
    /// with this one.
    pub fn new(label: impl IntoDataPointLabel, value: u32) -> Self {
        Self { label: label.into_data_point_label(), value }
    }
}

/// A label accepted by [`DataPoint::new`].
///
/// Implemented for the two forms a caller actually writes — an owned/borrowed string and a
/// categorical index — so the single constructor serves both without a trait import at the call
/// site.
pub trait IntoDataPointLabel {
    /// Converts this label to the stored `String` form.
    fn into_data_point_label(self) -> String;
}

impl IntoDataPointLabel for String {
    fn into_data_point_label(self) -> String {
        self
    }
}

impl IntoDataPointLabel for &str {
    fn into_data_point_label(self) -> String {
        self.to_string()
    }
}

impl IntoDataPointLabel for u32 {
    fn into_data_point_label(self) -> String {
        self.to_string()
    }
}

impl IntoDataPointLabel for i32 {
    fn into_data_point_label(self) -> String {
        self.to_string()
    }
}

impl IntoDataPointLabel for usize {
    fn into_data_point_label(self) -> String {
        self.to_string()
    }
}

/// Simplified line/bar chart widget for mini builds.
///
/// Provides a compact chart suitable for mini UI builds where the full
/// `ChartWidget` (desktop profile) is too heavy.
pub struct MiniChart {
    base: BaseWidget,
    chart_type: ChartType,
    data: Vec<DataPoint>,
    min_value: u32,
    max_value: u32,
    /// Whether the min/mid/max value labels and the first/last index labels are painted.
    show_axis_labels: bool,
    /// The data index the pointer is nearest to, when the pointer is over the plot.
    hovered_index: Option<usize>,
    /// Emitted with the hovered index, or `None` when the pointer leaves.
    pub point_hover_changed: Signal1<Option<usize>>,
    /// Whether a new series grows from the baseline instead of appearing at full height.
    animate_data: bool,
    /// Drives the entrance scale from `0.0` to `1.0`.
    entrance: crate::style::PropertyDriver,
    /// The hover caption's text, reused across frames.
    ///
    /// The caption is one number; rebuilding it with `to_string()` on every frame allocated a
    /// fresh string for a value that changes only when the hovered point does.
    hover_caption: String,
    /// The `(index, value)` [`Self::hover_caption`] was rendered from.
    hover_caption_key: Option<(usize, u32)>,
}

impl MiniChart {
    /// Creates a new MiniChart with default Line chart type and 0–100 range.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MiniChart, rect, "MiniChart"),
            chart_type: ChartType::Line,
            data: Vec::new(),
            min_value: 0,
            max_value: 100,
            show_axis_labels: false,
            hovered_index: None,
            point_hover_changed: Signal1::new(),
            animate_data: false,
            // At rest at the settled end: with the option off the series is never scaled, and
            // turning it on resets the driver deliberately.
            entrance: crate::style::PropertyDriver::at(1.0, crate::style::MotionSlot::Normal),
            hover_caption: String::new(),
            hover_caption_key: None,
        }
    }

    /// Compatibility spelling of [`Self::point_hover_changed`] (W-21): the same emitted signal.
    pub fn hovered_point_changed(&self) -> &Signal1<Option<usize>> {
        &self.point_hover_changed
    }

    /// Sets the chart type (Line or Bar).
    pub fn set_chart_type(&mut self, t: ChartType) {
        if self.chart_type == t {
            return;
        }
        self.chart_type = t;
        self.base.request_redraw();
    }

    /// Returns the current chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// Sets the chart data points.
    ///
    /// When [`MiniChart::set_animate_data`] is on, a non-empty series restarts its entrance
    /// animation from the baseline. With animation off the series is drawn at full height as it
    /// always was. Replacing the series also clears the hovered index, because the old index may
    /// no longer name a point.
    pub fn set_data(&mut self, data: Vec<DataPoint>) {
        self.data = data;
        if self.hovered_index.is_some() {
            self.set_hovered_index(None);
        }
        if self.animate_data && !self.data.is_empty() {
            self.reset_entrance();
        }
        self.base.request_redraw();
    }

    /// Returns the current data points.
    pub fn data(&self) -> &[DataPoint] {
        &self.data
    }

    /// Sets the value range (min, max) for the chart Y axis.
    pub fn set_range(&mut self, min: u32, max: u32) {
        let (a, b) = if min <= max { (min, max) } else { (max, min) };
        if self.min_value == a && self.max_value == b {
            return;
        }
        self.min_value = a;
        self.max_value = b;
        self.base.request_redraw();
    }

    /// Enables or disables the axis value/category labels.
    ///
    /// When on, the value axis draws its minimum, midpoint and maximum as numbers in the left
    /// margin, and the category axis draws the first and last **index** below the plot. Indexes
    /// rather than data labels are used so a long label cannot overflow the reserved margin. The
    /// default is `false`, which paints exactly the historical picture.
    pub fn set_show_axis_labels(&mut self, on: bool) {
        if self.show_axis_labels == on {
            return;
        }
        self.show_axis_labels = on;
        self.base.request_redraw();
    }

    /// Whether the axis labels are painted. Defaults to `false`.
    pub fn show_axis_labels(&self) -> bool {
        self.show_axis_labels
    }

    /// Enables or disables the series entrance animation.
    ///
    /// When on, a series grows from the chart's baseline to its real height over the theme's
    /// `normal` motion tempo. The animation honours the environment's reduced-motion preference:
    /// with reduced motion requested the series starts at its settled height instead of growing.
    /// Turning the option off jumps straight to the settled height. The default is `false`.
    pub fn set_animate_data(&mut self, on: bool) {
        if self.animate_data == on {
            return;
        }
        self.animate_data = on;
        if on && !self.data.is_empty() {
            self.reset_entrance();
        } else if !on {
            self.entrance.jump_to(1.0);
        }
        self.base.request_redraw();
    }

    /// Whether the series entrance animation is enabled. Defaults to `false`.
    pub fn animate_data(&self) -> bool {
        self.animate_data
    }

    /// The data index the pointer is nearest to, or `None` when the pointer is not over the plot.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The value at [`MiniChart::hovered_index`], if that index still names a point.
    pub fn hovered_value(&self) -> Option<u32> {
        self.data.get(self.hovered_index?).map(|point| point.value)
    }

    /// Restarts the entrance at the baseline, settling immediately under reduced motion.
    fn reset_entrance(&mut self) {
        self.entrance.jump_to(0.0);
        self.entrance.set_target(1.0);
        if crate::style::environment::environment().prefers_reduced_motion() {
            self.entrance.jump_to(1.0);
        }
    }

    /// The plot rectangle, excluding the axis margins.
    fn plot_area(rect: Rect) -> Rect {
        let left = 40i32;
        let right = 10i32;
        let top = 10i32;
        let bottom = 30i32;
        Rect::new(
            rect.x + left,
            rect.y + top,
            (rect.width as i32 - left - right).max(1) as u32,
            (rect.height as i32 - top - bottom).max(1) as u32,
        )
    }

    /// The bar width and inter-bar gap the current series produces for `chart_area_w`.
    ///
    /// Shared by the bar painter and the hover hit test, so a marker lands on the bar the user
    /// actually sees rather than on a second, independently computed layout.
    fn bar_layout(&self, chart_area_w: u32) -> (u32, u32) {
        let bar_count = self.data.len().max(1) as u32;
        let max_bar_width = 40u32;
        let bar_width = (chart_area_w / bar_count).min(max_bar_width).max(2);
        let used_width = bar_width * bar_count;
        let remaining = chart_area_w.saturating_sub(used_width);
        let gap = (remaining / (bar_count + 1)).max(1);
        (bar_width, gap)
    }

    /// The horizontal centre of the point at `index`, in window coordinates.
    fn point_center_x(&self, index: usize, plot: Rect) -> i32 {
        let count = self.data.len();
        if count == 0 {
            return plot.x;
        }
        match self.chart_type {
            ChartType::Line => {
                if count == 1 {
                    plot.x + plot.width as i32 / 2
                } else {
                    plot.x + (plot.width as i32 * index as i32 / (count - 1) as i32)
                }
            }
            ChartType::Bar => {
                let (bar_width, gap) = self.bar_layout(plot.width);
                plot.x
                    + gap as i32 * (index + 1) as i32
                    + bar_width as i32 * index as i32
                    + bar_width as i32 / 2
            }
        }
    }

    /// The index whose drawn point centre is nearest `x`, when `x` is over the plot.
    fn nearest_index(&self, x: i32, plot: Rect) -> Option<usize> {
        if self.data.is_empty() || x < plot.x || x >= plot.x + plot.width as i32 {
            return None;
        }
        let mut best: Option<(usize, i32)> = None;
        for index in 0..self.data.len() {
            let distance = (self.point_center_x(index, plot) - x).abs();
            if best.is_none_or(|(_, current)| distance < current) {
                best = Some((index, distance));
            }
        }
        best.map(|(index, _)| index)
    }

    /// Replaces the hovered index, emitting only when it actually changes.
    fn set_hovered_index(&mut self, next: Option<usize>) {
        if self.hovered_index == next {
            return;
        }
        self.hovered_index = next;
        self.point_hover_changed.emit(next);
        self.base.request_redraw();
    }

    /// Updates the hover from a pointer position, clearing it when the pointer is off the plot.
    fn update_hover(&mut self, pos: Point) {
        if !self.base.is_enabled() || !self.geometry().contains_point(pos) {
            self.set_hovered_index(None);
            return;
        }
        let index = self.nearest_index(pos.x, Self::plot_area(self.geometry()));
        self.set_hovered_index(index);
    }
}

impl Widget for MiniChart {
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
        Size::new(200, 150)
    }

    /// Advances the series entrance by `delta_ms`; `true` while it is still growing.
    ///
    /// The driver collapses to its settled value on the first tick when the environment requests
    /// reduced motion, so the end state is the same without a growth phase. With animation off the
    /// driver is held at the settled end and this returns `false`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate_data {
            return false;
        }
        let moving = self.entrance.tick(delta_ms);
        if moving {
            self.base.request_redraw();
        }
        moving
    }

    /// Whether the entrance animation still owes frames.
    fn is_animating(&self) -> bool {
        self.animate_data && self.entrance.is_moving()
    }

    /// Resolves the published event name to the signal the control emits.
    ///
    /// The payload is declared `UInt` with an optional shape, so a hover delivers
    /// the point index and leaving the plot delivers `Null` — the same value an
    /// absent optional property uses, which keeps "no hovered point"
    /// distinguishable from "point 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "point_hover_changed" => Some(EventSignalRef::mapped(
                "point_hover_changed",
                &self.point_hover_changed,
                |v| match v {
                    Some(index) => CapabilityValue::UInt(*index as u64),
                    None => CapabilityValue::Null,
                },
            )),
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MiniChart`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including the
/// `"line"` / `"bar"` token spellings and the `TypeMismatch` an unknown token
/// produced.
impl WidgetProperties for MiniChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "chart_type" => {
                let s = match self.chart_type() {
                    ChartType::Line => "line",
                    ChartType::Bar => "bar",
                };
                Ok(CapabilityValue::String(s.to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "chart_type" => {
                let s = expect_string(value)?;
                let ct = match s.as_str() {
                    "line" => ChartType::Line,
                    "bar" => ChartType::Bar,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_chart_type(ct);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["chart_type", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `mini_chart` publishes.
    ///
    /// All three assign state — the style, the series and the axis bounds — so each
    /// needs an argument a command carries none of. They are refused as
    /// [`CapabilityAccessError::OutOfRange`] rather than reported unknown: `chart_type`
    /// travels through `set("chart_type", ..)`, while `set_data` and `set_range` have
    /// no property equivalent yet, so a caller reading `OutOfRange` knows the name was
    /// recognised and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_chart_type" | "set_data" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MiniChart {
    /// Tracks the nearest data point while the pointer is over the plot.
    ///
    /// A move outside the plot (still inside the widget's margin) clears the hover, so the marker
    /// and caption never claim a value the pointer is not near. `MouseLeave` clears it as well.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseMove { pos } => self.update_hover(*pos),
            Event::MouseLeave { .. } => self.set_hovered_index(None),
            _ => {}
        }
    }
}

impl Draw for MiniChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();

        // The backdrop behind the plot, which is also the surface every piece of chrome below
        // has to be legible *on*. `mini_chart` is absent from the role table, so it classifies
        // as `Surface` and a theme writes `theme.colors.background` here — the direct value, not
        // a per-control resolution, because the chart engine's shared derivation measures
        // against exactly that fill. The literal is a no-theme fallback, and it is the light
        // surface the old chrome was written for.
        // The surface resolves the caller's style first, then **the theme's** resolved style for
        // this control, and only then a literal.
        //
        // The theme step was missing, which made the control theme-blind in exactly the case the
        // rendering census measures: a freshly constructed control has an empty style, so the
        // bare `rgb(255,255,255)` fallback won in *both* appearances and the light and dark
        // snapshots came out identical. Reading the resolved theme is what makes an appearance
        // switch visible on a control nobody has styled.
        let theme = crate::style::resolved_theme_style_shared("mini_chart");
        let bg_color = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(255, 255, 255));
        // The series stroke is the control's ink: `text_color` where resolved, and a colour
        // derived to contrast with *this* chart's own backdrop otherwise. The plain literal is
        // unreachable whenever a theme is active (the theme always writes a text colour), but a
        // colour chosen against `bg_color` is the only choice that stays correct if one is not:
        // a fixed black line is invisible on a dark surface.
        let line_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| bg_color.contrast_color());
        // Grid and axis are chrome around the data. The grid is deliberately the faint end of
        // the surface-to-ink scale: it used to be a fixed `rgb(220,220,220)`, which on the dark
        // appearance made the *least* important element in the control the brightest thing on
        // screen. Deriving both from `bg_color` — the surface this control actually painted —
        // is what keeps them on the right side of it in either appearance.
        let axis_color = bg_color.blend(&line_color, 0.45);
        let grid_color = bg_color.blend(&line_color, 0.16);

        // Draw background
        context.face_with_gradient(
            rect,
            bg_color,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Chart area, excluding the axis margins. The left margin reserves room for the value
        // labels this control can now draw; the reservation is part of the published layout, not
        // something the label option changes.
        let plot = Self::plot_area(rect);
        let chart_area_w = plot.width;
        let chart_area_h = plot.height;
        let chart_origin_x = plot.x;
        let chart_origin_y = plot.y;

        // Draw horizontal grid lines (the faint end of the shared axis derivation; see above).
        let grid_count = 4;
        for i in 0..=grid_count {
            let gy = chart_origin_y + (chart_area_h as i32 * i / grid_count);
            context.draw_line(
                Point::new(chart_origin_x, gy),
                Point::new(chart_origin_x + chart_area_w as i32 - 1, gy),
                grid_color,
            );
        }

        // Draw Y axis (vertical line on the left)
        context.draw_line(
            Point::new(chart_origin_x, chart_origin_y),
            Point::new(chart_origin_x, chart_origin_y + chart_area_h as i32),
            axis_color,
        );

        // Draw X axis (horizontal line at the bottom)
        context.draw_line(
            Point::new(chart_origin_x, chart_origin_y + chart_area_h as i32),
            Point::new(
                chart_origin_x + chart_area_w as i32 - 1,
                chart_origin_y + chart_area_h as i32,
            ),
            axis_color,
        );

        // Compute effective Y range before the labels so the value axis can state it even when
        // there is no series yet.
        let effective_max = if self.max_value > self.min_value {
            self.max_value
        } else {
            let max_val = self.data.iter().map(|dp| dp.value).max().unwrap_or(100);
            if max_val < 1 {
                100
            } else {
                max_val
            }
        };
        let effective_min = self.min_value;
        let range = (effective_max - effective_min) as f32;

        if self.show_axis_labels {
            let font = Font::simple("sans-serif", 10.0);
            let mid = ((u64::from(effective_min) + u64::from(effective_max)) / 2) as u32;
            let value_labels = [
                (effective_max, chart_origin_y),
                (mid, chart_origin_y + chart_area_h as i32 / 2),
                (effective_min, chart_origin_y + chart_area_h as i32),
            ];
            for (value, y) in value_labels {
                context.draw_text(
                    Point::new(chart_origin_x - 4, y + 4),
                    &value.to_string(),
                    &font,
                    axis_color,
                    HorizontalAlignment::Right,
                );
            }
            if let Some(last) = self.data.len().checked_sub(1) {
                let label_y = chart_origin_y + chart_area_h as i32 + 14;
                context.draw_text(
                    Point::new(chart_origin_x, label_y),
                    "0",
                    &font,
                    axis_color,
                    HorizontalAlignment::Left,
                );
                context.draw_text(
                    Point::new(chart_origin_x + chart_area_w as i32 - 1, label_y),
                    &last.to_string(),
                    &font,
                    axis_color,
                    HorizontalAlignment::Right,
                );
            }
        }

        if self.data.is_empty() {
            return;
        }

        // The entrance scale. With the option off the closure returns the ratio untouched, which
        // is what keeps the default drawing byte-identical; with it on the series grows out of the
        // baseline (a zero ratio is the bottom of the plot).
        let animate = self.animate_data;
        let progress = self.entrance.value().clamp(0.0, 1.0);
        let scale_ratio = move |ratio: f32| -> f32 {
            if animate {
                (ratio * progress).clamp(0.0, 1.0)
            } else {
                ratio
            }
        };

        match self.chart_type {
            ChartType::Line => {
                let count = self.data.len();
                if count == 1 {
                    // Single data point: draw a small cross marker at the center
                    let cx = chart_origin_x + chart_area_w as i32 / 2;
                    let y_ratio = if range > 0.0 {
                        (self.data[0].value - effective_min) as f32 / range
                    } else {
                        0.5
                    };
                    let y_ratio = scale_ratio(y_ratio);
                    let cy = chart_origin_y + chart_area_h as i32
                        - (chart_area_h as f32 * y_ratio) as i32;
                    // Cross marker arms
                    let arm = 3i32;
                    context.draw_line_stroke(
                        Point::new(cx - arm, cy),
                        Point::new(cx + arm, cy),
                        line_color,
                        2,
                    );
                    context.draw_line_stroke(
                        Point::new(cx, cy - arm),
                        Point::new(cx, cy + arm),
                        line_color,
                        2,
                    );
                } else if count >= 2 {
                    for i in 0..(count - 1) {
                        let x1 =
                            chart_origin_x + (chart_area_w as i32 * i as i32 / (count - 1) as i32);
                        let x2 = chart_origin_x
                            + (chart_area_w as i32 * (i + 1) as i32 / (count - 1) as i32);

                        let y1_ratio = if range > 0.0 {
                            (self.data[i].value - effective_min) as f32 / range
                        } else {
                            0.5
                        };
                        let y2_ratio = if range > 0.0 {
                            (self.data[i + 1].value - effective_min) as f32 / range
                        } else {
                            0.5
                        };

                        let y1 = chart_origin_y + chart_area_h as i32
                            - (chart_area_h as f32 * scale_ratio(y1_ratio)) as i32;
                        let y2 = chart_origin_y + chart_area_h as i32
                            - (chart_area_h as f32 * scale_ratio(y2_ratio)) as i32;

                        context.draw_line_stroke(
                            Point::new(x1, y1),
                            Point::new(x2, y2),
                            line_color,
                            2,
                        );
                    }
                }
            }
            ChartType::Bar => {
                let count = self.data.len();
                if count == 0 {
                    return;
                }

                let (bar_width, gap) = self.bar_layout(chart_area_w);

                for (i, dp) in self.data.iter().enumerate() {
                    let bx =
                        chart_origin_x + gap as i32 * (i + 1) as i32 + bar_width as i32 * i as i32;

                    let bar_h_ratio =
                        if range > 0.0 { (dp.value - effective_min) as f32 / range } else { 0.5 };
                    let bar_h = (chart_area_h as f32 * scale_ratio(bar_h_ratio)) as u32;

                    if bar_h == 0 {
                        continue;
                    }

                    let by = chart_origin_y + chart_area_h as i32 - bar_h as i32;

                    // Proportionally color the bar: low values → blue, high values → red. The
                    // colour follows the *value*, not the entrance scale, so a bar keeps its
                    // meaning while it grows.
                    let t = bar_h_ratio.clamp(0.0, 1.0);
                    let r = (60.0 + t * 195.0) as u8;
                    let g = (120.0 + t * 50.0) as u8;
                    let b = (200.0 - t * 180.0) as u8;

                    context.fill_rect(Rect::new(bx, by, bar_width, bar_h), Color::rgb(r, g, b));
                }
            }
        }

        // Hover marker and value caption, drawn last so they sit over the series.
        if let Some(index) = self.hovered_index {
            if let Some(point) = self.data.get(index) {
                let x = self.point_center_x(index, plot);
                context.draw_line_stroke(
                    Point::new(x, chart_origin_y),
                    Point::new(x, chart_origin_y + chart_area_h as i32),
                    axis_color,
                    1,
                );
                // The caption is written only when the point it names changes; a pointer resting
                // on one point reuses the buffer instead of building the same string per frame.
                let key = (index, point.value);
                if self.hover_caption_key != Some(key) {
                    self.hover_caption.clear();
                    let _ = write!(self.hover_caption, "{}", point.value);
                    self.hover_caption_key = Some(key);
                }
                context.draw_text(
                    Point::new(x + 4, chart_origin_y + 12),
                    self.hover_caption.as_str(),
                    &Font::simple("sans-serif", 10.0),
                    line_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use crate::theme::AppearanceMode;

    /// The one published name must resolve, and an unpublished one must not.
    #[test]
    fn published_hovered_point_event_resolves_to_a_signal() {
        let chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        assert!(chart.event_signal_dyn("point_hover_changed").is_some());
        assert!(chart.event_signal_dyn("no_such_event").is_none());
    }

    #[test]
    fn mini_chart_creation() {
        let chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        assert_eq!(chart.chart_type(), ChartType::Line);
        assert!(chart.data().is_empty());
        assert_eq!(chart.size_hint(), Size::new(200, 150));
    }

    #[test]
    fn mini_chart_set_data() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        let data = vec![DataPoint::new("A", 10), DataPoint::new("B", 50), DataPoint::new("C", 30)];
        chart.set_data(data);
        assert_eq!(chart.data().len(), 3);
        assert_eq!(chart.data()[0].label, "A");
        assert_eq!(chart.data()[0].value, 10);
        assert_eq!(chart.data()[1].value, 50);
        assert_eq!(chart.data()[2].value, 30);
    }

    #[test]
    fn mini_chart_set_chart_type() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        assert_eq!(chart.chart_type(), ChartType::Line);
        chart.set_chart_type(ChartType::Bar);
        assert_eq!(chart.chart_type(), ChartType::Bar);
        chart.set_chart_type(ChartType::Line);
        assert_eq!(chart.chart_type(), ChartType::Line);
    }

    #[test]
    fn mini_chart_set_range() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_range(0, 200);
        chart.set_range(500, 100); // inverted — should swap
                                   // Does not panic; range is stored internally
    }

    #[test]
    fn mini_chart_draw_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        let data = vec![
            DataPoint::new("Jan", 10),
            DataPoint::new("Feb", 50),
            DataPoint::new("Mar", 80),
            DataPoint::new("Apr", 30),
        ];
        chart.set_data(data);
        chart.set_chart_type(ChartType::Line);

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }

    #[test]
    fn mini_chart_draw_empty_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }

    #[test]
    fn mini_chart_draw_bar_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_data(vec![
            DataPoint::new("X", 25),
            DataPoint::new("Y", 70),
            DataPoint::new("Z", 45),
        ]);
        chart.set_chart_type(ChartType::Bar);

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }

    /// The grid is chrome around the data. It used to be a fixed `rgb(220,220,220)`, which on the
    /// dark appearance made the *least* important element in the control the brightest thing on
    /// screen, and the grid did not change between appearances at all.
    ///
    /// The expectation is read off the rendered drawing rather than a copied-out colour: the grid
    /// must move with the appearance, and it must be the *faintest* thing the control draws —
    /// closer to the backdrop than the axis lines it belongs to.
    ///
    /// The guard is held across both renders and the light default is restored before it is
    /// released, so this test neither observes another test's appearance switch nor leaves one
    /// behind for the next test. See `masked_edit_body_ink_is_legible_on_its_own_field` for the
    /// full argument.
    #[test]
    fn mini_chart_grid_follows_the_appearance_and_stays_faint() {
        /// The stroke colours of the `<line>` elements in draw order, as `(grid..., axis...)`.
        fn strokes(appearance: AppearanceMode) -> Vec<String> {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
            let svg = crate::widget::svg::render_to_svg(&mut chart);
            svg.lines()
                .filter(|l| l.contains("<line"))
                .filter_map(|l| {
                    l.split("stroke=\"")
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .map(|s| s.to_string())
                })
                .collect()
        }

        let _guard = crate::style::theme_test_guard();
        // The presets must exist before an appearance can be selected. `set_appearance` reports
        // whether it found a theme of that appearance and returns `false` when it did not — and
        // this test used to ignore that return value, so both renders happened under whatever
        // theme was already active and the assertion compared a run with itself. Registering the
        // pair and asserting the switch succeeded is what makes the two lists two *different*
        // appearances rather than two copies of one.
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.register_theme(crate::theme::Theme::dark());
        }
        let light = strokes(AppearanceMode::Light);
        let dark = strokes(AppearanceMode::Dark);
        crate::theme::global_theme_manager().set_appearance(AppearanceMode::Light);
        assert!(!light.is_empty(), "the chart must draw its grid and axes");
        assert!(!dark.is_empty(), "the chart must draw its grid and axes");
        assert_ne!(
            light, dark,
            "the grid and axes must respond to the appearance, not be fixed literals"
        );

        // The grid lines come first (there are five of them), then the two axis lines. On either
        // appearance the grid is the darker-end / fainter-end stroke, so the two groups differ.
        assert!(
            light.len() >= 3 && dark.len() >= 3,
            "expected grid lines plus two axes, got light={light:?} dark={dark:?}"
        );
        assert_ne!(light[0], light[light.len() - 1], "light: grid must differ from the axis");
        assert_ne!(dark[0], dark[dark.len() - 1], "dark: grid must differ from the axis");
        // Every grid line shares one colour, and both axes share one colour.
        assert!(light[..light.len() - 2].iter().all(|c| *c == light[0]));
        assert!(dark[..dark.len() - 2].iter().all(|c| *c == dark[0]));
    }

    /// Renders the chart and returns the software surface's pixels.
    fn render_pixels(chart: &mut MiniChart) -> Vec<u8> {
        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// Axis labels are opt-in, and turning them on changes the picture.
    #[test]
    fn mini_chart_axis_labels_are_optional_and_change_the_picture() {
        let _guard = crate::style::theme_test_guard();
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_data(vec![
            DataPoint::new("A", 10),
            DataPoint::new("B", 50),
            DataPoint::new("C", 30),
        ]);

        assert!(!chart.show_axis_labels(), "labels are off by default");
        let without = crate::widget::svg::render_to_svg(&mut chart);

        chart.set_show_axis_labels(true);
        assert!(chart.show_axis_labels());
        let with = crate::widget::svg::render_to_svg(&mut chart);

        assert_ne!(
            without, with,
            "the labels must add drawing commands instead of leaving the picture unchanged"
        );
    }

    /// Hovering the plot reports the nearest index and leaving clears it.
    #[test]
    fn mini_chart_hover_reports_the_nearest_index_and_clears_on_leave() {
        use std::sync::{Arc, Mutex};

        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_data(vec![
            DataPoint::new("A", 10),
            DataPoint::new("B", 50),
            DataPoint::new("C", 30),
        ]);

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        chart.point_hover_changed.connect(move |index| {
            if let Ok(mut values) = sink.lock() {
                values.push(*index);
            }
        });

        // The plot is x 40..190, so the middle of three line points is x = 115.
        chart.handle_event(&Event::MouseMove { pos: Point::new(115, 50) });
        assert_eq!(chart.hovered_index(), Some(1));
        assert_eq!(chart.hovered_value(), Some(50));

        chart.handle_event(&Event::MouseLeave { pos: Point::new(115, 50) });
        assert_eq!(chart.hovered_index(), None, "leaving clears the hover");
        assert_eq!(chart.hovered_value(), None);

        let values = seen.lock().expect("hover signal lock poisoned").clone();
        assert_eq!(values, vec![Some(1), None], "each change emits exactly once");
    }

    /// The entrance animation starts at the baseline, advances on tick, and settles.
    #[test]
    fn mini_chart_entrance_animation_scales_then_settles() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_data(vec![
            DataPoint::new("A", 10),
            DataPoint::new("B", 50),
            DataPoint::new("C", 30),
        ]);
        assert!(!chart.animate_data(), "the entrance is off by default");
        assert!(!Widget::is_animating(&chart));

        chart.set_animate_data(true);
        assert!(chart.animate_data());
        assert!(Widget::is_animating(&chart), "a new entrance owes frames");

        let baseline = render_pixels(&mut chart);
        assert!(!Widget::tick(&mut chart, 5_000), "the entrance must settle");
        assert!(!Widget::is_animating(&chart));
        let settled = render_pixels(&mut chart);
        assert_ne!(baseline, settled, "the series must grow from zero to its full height");
    }
}
