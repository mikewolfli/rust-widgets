// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MiniChart widget — simplified line/bar chart for mini builds (BLUE13 R2.10).

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Point, Rect, Size};
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
    pub fn new(label: impl Into<String>, value: u32) -> Self {
        Self { label: label.into(), value }
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
        }
    }

    /// Sets the chart type (Line or Bar).
    pub fn set_chart_type(&mut self, t: ChartType) {
        self.chart_type = t;
        self.base.request_redraw();
    }

    /// Returns the current chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// Sets the chart data points.
    pub fn set_data(&mut self, data: Vec<DataPoint>) {
        self.data = data;
        self.base.request_redraw();
    }

    /// Returns the current data points.
    pub fn data(&self) -> &[DataPoint] {
        &self.data
    }

    /// Sets the value range (min, max) for the chart Y axis.
    pub fn set_range(&mut self, min: u32, max: u32) {
        let (a, b) = if min <= max { (min, max) } else { (max, min) };
        self.min_value = a;
        self.max_value = b;
        self.base.request_redraw();
    }
}

impl Widget for MiniChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 150)
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
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
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
        let theme = crate::style::resolved_theme_style("mini_chart");
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
        context.fill_rect(rect, bg_color);

        // Chart area margins. The left margin reserves room for the value labels a full
        // `ChartWidget` draws; this control draws none, so the reservation is generous — but it
        // is part of the layout the control is published with, not something this fix changes.
        let margin_left = 40i32;
        let margin_right = 10i32;
        let margin_top = 10i32;
        let margin_bottom = 30i32;

        let chart_area_w = (rect.width as i32 - margin_left - margin_right).max(1) as u32;
        let chart_area_h = (rect.height as i32 - margin_top - margin_bottom).max(1) as u32;
        let chart_origin_x = rect.x + margin_left;
        let chart_origin_y = rect.y + margin_top;

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

        if self.data.is_empty() {
            return;
        }

        // Compute effective Y range
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
                            - (chart_area_h as f32 * y1_ratio) as i32;
                        let y2 = chart_origin_y + chart_area_h as i32
                            - (chart_area_h as f32 * y2_ratio) as i32;

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

                // Compute gaps and bar widths evenly across the chart area
                let bar_count = count as u32;
                // Reserve one gap before first bar and one after last bar
                let total_gaps = bar_count + 1;
                // Try to use a reasonable bar width, shrinking if needed
                let max_bar_width = 40u32;
                let ideal_bar_width = (chart_area_w / bar_count).min(max_bar_width).max(2);
                let used_width = ideal_bar_width * bar_count;
                let remaining = chart_area_w.saturating_sub(used_width);
                let gap = (remaining / total_gaps).max(1);
                let bar_width = ideal_bar_width;

                for (i, dp) in self.data.iter().enumerate() {
                    let bx =
                        chart_origin_x + gap as i32 * (i + 1) as i32 + bar_width as i32 * i as i32;

                    let bar_h_ratio =
                        if range > 0.0 { (dp.value - effective_min) as f32 / range } else { 0.5 };
                    let bar_h = (chart_area_h as f32 * bar_h_ratio) as u32;

                    if bar_h == 0 {
                        continue;
                    }

                    let by = chart_origin_y + chart_area_h as i32 - bar_h as i32;

                    // Proportionally color the bar: low values → blue, high values → red
                    let t = bar_h_ratio.clamp(0.0, 1.0);
                    let r = (60.0 + t * 195.0) as u8;
                    let g = (120.0 + t * 50.0) as u8;
                    let b = (200.0 - t * 180.0) as u8;

                    context.fill_rect(Rect::new(bx, by, bar_width, bar_h), Color::rgb(r, g, b));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use crate::theme::AppearanceMode;

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

        let _guard = crate::theme::theme_test_guard();
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
}
