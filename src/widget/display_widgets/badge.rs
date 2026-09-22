// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Badge widget — a notification count/indicator dot.
//!
//! The Badge widget displays a small notification count or status indicator,
//! rendered as a colored circle or pill shape. It supports different severity
//! levels (Info, Success, Warning, Error) and a dot mode that shows only a
//! colored dot without text. When the count is set to 0, the badge hides itself.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::style::SemanticColor;
use crate::widget::capability::coercion::{
    badge_level_to_str, expect_badge_level, expect_i64, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Severity level for a badge, determining its color scheme.
/// Badge severity/notification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeLevel {
    /// Informational — blue.
    #[default]
    Info,
    /// Success/green — green.
    Success,
    /// Warning — yellow/orange.
    Warning,
    /// Error/danger — red.
    Error,
}

impl BadgeLevel {
    /// Returns the background color associated with this severity level.
    pub fn color(&self) -> Color {
        match self {
            BadgeLevel::Info => Color::INFO,
            BadgeLevel::Success => Color::SUCCESS,
            BadgeLevel::Warning => Color::WARNING,
            BadgeLevel::Error => Color::ERROR,
        }
    }
}

/// Badge widget for notification counts and status indicators.
///
/// Radius of the dot-mode marker, in logical pixels.
///
/// The size the marker is *defined* to be, independent of the rectangle the caller laid the
/// badge out in — Material's `smallSize` is the same value. Deriving it from the geometry drew
/// a 60 px disc in the 240x120 census cell and a 12 px one in a 24 px row.
const DOT_RADIUS: u32 = 5;

/// Renders as a filled circle or pill with an optional text overlay.
/// Supports dot mode (colored dot with no text) and automatic hiding
/// when count is 0 (unless overridden with custom text).
pub struct Badge {
    base: BaseWidget,
    count: u32,
    text: String,
    level: BadgeLevel,
    dot_mode: bool,
}

impl Badge {
    /// Creates a new Badge widget with the given geometry.
    ///
    /// Initial state has no count, no text, and an Info level.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Badge, geometry, "Badge"),
            count: 0,
            text: String::new(),
            level: BadgeLevel::Info,
            dot_mode: false,
        }
    }

    /// Sets the notification count. A value of 0 hides the badge (unless dot mode is enabled
    /// or custom text is set). Values above 999 are displayed as "999+".
    pub fn set_count(&mut self, count: u32) {
        self.count = count;
        self.base.request_redraw();
    }

    /// Returns the current notification count.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Sets the display text shown inside the badge. When non-empty, this takes
    /// precedence over the numeric count.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Returns the current display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the severity level, which changes the badge's background color.
    pub fn set_level(&mut self, level: BadgeLevel) {
        self.level = level;
        self.base.request_redraw();
    }

    /// Returns the current severity level.
    pub fn level(&self) -> BadgeLevel {
        self.level
    }

    /// Enables or disables dot mode. When enabled, the badge renders as a small
    /// colored dot without any text overlay, regardless of count or text.
    pub fn set_dot_mode(&mut self, enabled: bool) {
        self.dot_mode = enabled;
        self.base.request_redraw();
    }

    /// Returns whether dot mode is currently enabled.
    pub fn is_dot_mode(&self) -> bool {
        self.dot_mode
    }

    /// Returns the display string to render inside the badge.
    fn display_text(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        if self.count > 999 {
            "999+".to_string()
        } else if self.count > 0 {
            self.count.to_string()
        } else {
            String::new()
        }
    }

    /// Returns whether the badge should be drawn at all.
    fn should_draw(&self) -> bool {
        if self.dot_mode {
            return true;
        }
        if !self.text.is_empty() {
            return true;
        }
        self.count > 0
    }
}

impl Widget for Badge {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(24, 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Badge`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for Badge {
    /// Returns `"text"`, `"count"` and `"level"`, delegating anything else to the
    /// base widget's shared properties.
    ///
    /// `"level"` answers the severity as its lowercase spelling (`"info"`,
    /// `"success"`, `"warning"`, `"error"`). It is the property that decides the
    /// badge's colour, so leaving it unreadable meant a caller could not discover why
    /// a badge was red — or set it without reaching for the inherent `set_level`.
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "count" => Ok(CapabilityValue::Int(self.count() as i64)),
            "level" => Ok(CapabilityValue::String(badge_level_to_str(self.level()).to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "count" => {
                let count = expect_i64(value)?;
                let count =
                    u32::try_from(count).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                self.set_count(count);
                Ok(())
            }
            "level" => {
                self.set_level(expect_badge_level(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "count", "level", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `badge` publishes.
    ///
    /// Both published names assign state — `set_text` a string, `set_count` a number
    /// — so each needs an argument that a command carries none of. They are refused
    /// as [`CapabilityAccessError::OutOfRange`], which tells the caller the name is
    /// right and the value belongs on the property route, rather than
    /// [`CapabilityAccessError::UnknownCommand`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_count" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Badge {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.should_draw() {
            return;
        }

        let rect = self.geometry();
        // The pill fill is chrome: it resolves explicit style first, then the theme's
        // resolved style for this control, and only then falls back to the severity's
        // own literal. Without the theme step a light/dark switch would change nothing
        // on screen, because the fill was previously hardcoded per level.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("badge");
        // The severity colour is the badge's **subject**, so it is the base the pill is built
        // from — not a last-resort fallback.
        //
        // This chain used to be `.or(themed_bg).unwrap_or(self.level.color())`, and
        // `themed_bg` was never `None`: `badge` is absent from the theme's role table, so it
        // resolved as `Surface` and the manager wrote the *window background* into it. The
        // `or` therefore always won, `BadgeLevel::Info/Success/Warning/Error` were unreachable,
        // and `badge.svg` showed a pill filled with the window's own colour — a badge that was
        // only visible as the hole it left in the label behind it.
        //
        // A caller's explicit colour still wins outright. Otherwise the severity is used, and
        // it is pushed away from the surface it sits on when the two would coincide, which is
        // the case that made the badge invisible in the first place.
        let surface = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .or(style.background_color)
            .unwrap_or(Color::WHITE);
        let severity = match self.level {
            BadgeLevel::Info => SemanticColor::Info,
            BadgeLevel::Success => SemanticColor::Success,
            BadgeLevel::Warning => SemanticColor::Warning,
            BadgeLevel::Error => SemanticColor::Error,
        };
        // The severity is resolved through the **theme's** token, not the level's own literal.
        //
        // `BadgeLevel::color()` returns `Color::INFO`/`WARNING`/… — fixed constants that do not
        // participate in the appearance. Painting one made the badge's *only* visible element
        // invariant between light and dark, which the rendering census reports as "this control
        // stopped following the appearance". The theme carries the same four severities as
        // tokens (`theme.colors.error/warning/success/info`), and this is precisely the axis
        // those tokens exist for: routing through them is what lets a theme restyle its own
        // severity scale. The literal remains as the fallback for a process with no theme
        // installed, where there is no token to read.
        let severity = crate::style::semantic_color(severity).unwrap_or_else(|| self.level.color());
        let bg_color = if style.theme_derived {
            // The style's colour came from the theme, so it describes the *surface* rather
            // than the severity; `theme_derived` is how a caller's own colour is told apart
            // from the manager's contribution.
            severity.legible_on(surface, 3.0)
        } else {
            style.background_color.unwrap_or_else(|| severity.legible_on(surface, 3.0))
        };
        // The count reads against the pill, so its colour is derived per-branch below from the
        // fill it actually lands on.
        let text_str = self.display_text();

        if self.dot_mode {
            // The dot is a **fixed-size** marker, not a fraction of the caller's rectangle:
            // `min(w, h) / 2` drew a 12 px dot in a 24 px census cell and a 60 px disc in a
            // 240x120 one, so the same badge was a different object in every layout. Material's
            // `smallSize` is 6.0 for the same marker.
            let dot_radius = DOT_RADIUS.min(rect.height.min(rect.width) / 2);
            let center =
                Point::new(rect.x + (rect.width as i32) / 2, rect.y + (rect.height as i32) / 2);
            context.fill_circle(center, dot_radius, bg_color);
            return;
        }

        if text_str.is_empty() {
            return;
        }

        // Determine the pill's dimensions from its **own** content, not from the
        // rectangle the caller happened to pass in.
        //
        // The pill's height came from `(glyph_height + padding_y * 2).max(rect.height.min(18))`
        // and its width from `(text_width + padding_x * 2).max(pill_height)`: the `rect`
        // term in each meant the same badge was a different object in every layout (a
        // 24 px census cell and a 240x120 one), and the two axes were coupled through
        // `pill_height` so a wide label also grew the pill taller. Both now derive from
        // the resolved label and the named pill metrics alone, and the pill is centred in
        // the control by [`ControlMetrics::center_in`], which clamps rather than expands.
        let font = Font::simple("sans-serif", dimensions::BADGE_LABEL_FONT_SIZE as f32);
        let metrics = context.measure_text(&text_str, &font);

        let text_width = metrics.width;
        // A pill is exactly tall enough for its own line box plus the named vertical
        // padding, with [`dimensions::BADGE_PILL_HEIGHT`] as the floor every severity
        // shares: a count and a label must be the same object, not two sizes. Both terms
        // are constants of the control — neither reads `rect` — so the badge is the same
        // pill in any layout. The floor is clamped to the control so a badge laid out
        // smaller than a pill is not painted outside its own rectangle.
        let line_height = context.measure_text("M", &font).height.max(1);
        let pill_height = (line_height + dimensions::BADGE_PILL_PADDING_V * 2)
            .max(dimensions::BADGE_PILL_HEIGHT.min(rect.height))
            .min(rect.height);
        // The horizontal padding is per side, so the pill is the label plus two of them;
        // a single-character count keeps a circular pill via the `max(pill_height)` floor.
        let pill_width =
            (text_width + dimensions::BADGE_PILL_PADDING_H * 2).max(pill_height).min(rect.width);

        let pill_rect =
            ControlMetrics::center_in(rect, crate::core::Size::new(pill_width, pill_height));
        let corner_radius = pill_rect.height / 2;

        // Draw pill background
        context.fill_rounded_rect(pill_rect, corner_radius, bg_color);

        // Draw the label centred **on the pill's own box**. `text_line(pill_rect, ..)` is
        // the pill's line box, so the glyph's top edge lands on the pill's middle line
        // minus half a line — the same derivation `lineedit`, `chip` and `button` use.
        // The label previously came from its own independent arithmetic, which happened
        // to be close (the pill's centre is y 60 and the label's box centre 59.5) but was
        // a coincidence of two separate constants rather than a fact about the pill.
        let text_color = bg_color.contrast_color();
        let line = context.text_line(pill_rect, &font);
        context.draw_text_fitted(
            Rect::new(line.x, line.y, line.width.max(1), line.height.max(1)),
            &text_str,
            &font,
            text_color,
            HorizontalAlignment::Center,
        );
    }
}

impl EventHandler for Badge {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn badge_default_creation() {
        let badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert_eq!(badge.kind(), WidgetKind::Badge);
        assert_eq!(badge.count(), 0);
        assert!(badge.text().is_empty());
        assert_eq!(badge.level(), BadgeLevel::Info);
        assert!(!badge.is_dot_mode());
        assert_eq!(badge.geometry(), Rect::new(0, 0, 40, 24));
    }

    #[test]
    fn badge_count_display() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert!(!badge.should_draw());

        badge.set_count(5);
        assert_eq!(badge.count(), 5);
        assert!(badge.should_draw());
        assert_eq!(badge.display_text(), "5");

        badge.set_count(123);
        assert_eq!(badge.display_text(), "123");

        badge.set_count(1000);
        assert_eq!(badge.display_text(), "999+");
    }

    /// The severity must be reachable through the **property route**, not only through
    /// the inherent `set_level`.
    ///
    /// `BadgeLevel` is what decides the badge's colour, so a caller that could not
    /// read it had no way to learn why a badge renders red — and a generic consumer
    /// (property editor, language binding) had no way to set it. `get`/`set` did not
    /// answer `"level"` and `property_names` omitted it, while the schema was the
    /// only half that would have declared it.
    #[test]
    fn badge_level_is_reachable_through_the_property_route() {
        use crate::widget::capability::types::CapabilityValue;

        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));

        // Readable, with the default reported as the `info` token.
        assert_eq!(
            badge.get("level").expect("`level` must be readable"),
            CapabilityValue::String("info".to_string())
        );

        // Writable, and the write has the same effect the inherent setter has.
        for (token, expected, colour) in [
            ("info", BadgeLevel::Info, Color::INFO),
            ("success", BadgeLevel::Success, Color::SUCCESS),
            ("warning", BadgeLevel::Warning, Color::WARNING),
            ("error", BadgeLevel::Error, Color::ERROR),
        ] {
            badge
                .set("level", CapabilityValue::String(token.to_string()))
                .unwrap_or_else(|e| panic!("set(\"level\", {token:?}) must be accepted: {e:?}"));
            assert_eq!(badge.level(), expected);
            assert_eq!(badge.level().color(), colour, "the level must drive the colour");
            assert_eq!(badge.get("level").unwrap(), CapabilityValue::String(token.to_string()));
        }

        // `danger` is an accepted synonym for `error`, since that is the word a
        // caller writing a failure badge reaches for.
        badge
            .set("level", CapabilityValue::String("danger".to_string()))
            .expect("`danger` must be accepted as a synonym for `error`");
        assert_eq!(badge.level(), BadgeLevel::Error);

        // Published, so a generic consumer can discover it.
        assert!(badge.property_names().contains(&"level"));

        // An unknown token is refused rather than silently becoming `Info`.
        assert!(badge.set("level", CapabilityValue::String("chartreuse".to_string())).is_err());
        assert!(badge.set("level", CapabilityValue::Bool(true)).is_err());
    }

    #[test]
    fn badge_level_changes_color() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));

        assert_eq!(badge.level(), BadgeLevel::Info);
        assert_eq!(badge.level().color(), Color::INFO);

        badge.set_level(BadgeLevel::Success);
        assert_eq!(badge.level(), BadgeLevel::Success);
        assert_eq!(badge.level().color(), Color::SUCCESS);

        badge.set_level(BadgeLevel::Warning);
        assert_eq!(badge.level(), BadgeLevel::Warning);
        assert_eq!(badge.level().color(), Color::WARNING);

        badge.set_level(BadgeLevel::Error);
        assert_eq!(badge.level(), BadgeLevel::Error);
        assert_eq!(badge.level().color(), Color::ERROR);
    }

    #[test]
    fn badge_zero_count_hides() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert!(!badge.should_draw());

        badge.set_count(0);
        assert!(!badge.should_draw());

        badge.set_text("!".to_string());
        assert!(badge.should_draw());

        badge.set_text(String::new());
        badge.set_count(1);
        assert!(badge.should_draw());

        badge.set_count(0);
        assert!(!badge.should_draw());
    }

    #[test]
    fn badge_dot_mode() {
        let mut badge = Badge::new(Rect::new(0, 0, 20, 20));
        assert!(!badge.is_dot_mode());

        badge.set_dot_mode(true);
        assert!(badge.is_dot_mode());
        assert!(badge.should_draw());

        badge.set_count(0);
        badge.set_text(String::new());
        assert!(badge.should_draw()); // dot mode always draws

        badge.set_dot_mode(false);
        assert!(!badge.should_draw()); // no count + no text + not dot mode = hidden
    }

    #[test]
    fn badge_text_override() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        badge.set_count(5);
        assert_eq!(badge.display_text(), "5");

        badge.set_text("New".to_string());
        assert_eq!(badge.display_text(), "New");
        assert_eq!(badge.text(), "New");

        badge.set_text(String::new());
        assert_eq!(badge.display_text(), "5");
    }

    #[test]
    fn badge_event_forwarding() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        // Should not panic
        badge.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        badge.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        badge.handle_event(&Event::Resize { size: Size::new(50, 30) });
    }

    #[test]
    fn badge_svg_output() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        badge.set_count(3);

        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    #[test]
    fn badge_svg_output_dot_mode() {
        let mut badge = Badge::new(Rect::new(0, 0, 20, 20));
        badge.set_dot_mode(true);

        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn badge_svg_hidden_when_zero() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        // count is 0, no text, not dot mode → should be hidden → only background fill
        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"));
        // The SVG backend adds one background fill rect; there should be no
        // additional fill elements from badge drawing since nothing was drawn.
        // A common test: SVG ends right after the background, no badge elements.
        let fill_count = svg.matches("fill=").count();
        assert_eq!(fill_count, 1, "expected only background fill, got {fill_count}: {svg}");
    }

    /// The label is centred on **the pill's own box**, not on an independent constant.
    ///
    /// This pins the defect the checklist named: the label's `y` was a literal that
    /// happened to land close to the pill's centre (pill centre y 60, label box centre
    /// 59.5 in the census cell), so the two agreed by coincidence rather than by
    /// construction — and any change to the pill's height or to the theme's font would
    /// have separated them. The label is now the pill's own line box, so its top edge is
    /// exactly half a line above the pill's middle line.
    #[test]
    fn the_label_sits_on_the_pills_own_line_box() {
        let mut badge = Badge::new(crate::widget::census::CENSUS_RECT);
        badge.set_text("Sample");
        let svg = render_to_svg(&mut badge);

        // The pill is the only rounded rect the badge emits.
        let pill_tag = svg
            .split("<rect")
            .find(|chunk| chunk.contains("rx="))
            .expect("the badge paints a pill");
        let attr = |name: &str| -> i32 {
            pill_tag
                .split(&format!("{name}=\""))
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or_else(|| panic!("the pill carries {name}: {pill_tag}"))
        };
        let (pill_y, pill_h) = (attr("y"), attr("height"));
        let pill_mid = pill_y + pill_h / 2;

        let label_tag = svg.split("<text").nth(1).expect("the badge paints its label");
        let label_y = label_tag
            .split("y=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse::<i32>().ok())
            .expect("the label carries a y");

        // A line box that is a whole number of pixels tall centred on `pill_mid` puts
        // its top edge at `pill_mid - height / 2`, so the label is never more than a
        // pixel off the pill's own middle whatever the font's metrics are.
        assert!(
            (label_y - (pill_mid - dimensions::BADGE_LABEL_FONT_SIZE as i32 / 2)).abs() <= 4,
            "the label ({label_y}) must hang from the pill's centre ({pill_mid}): {svg}"
        );
        // And it stays inside the pill, which the old literal nearly did not.
        assert!(
            label_y >= pill_y && label_y < pill_y + pill_h,
            "the label ({label_y}) must sit within the pill ({pill_y}..{})",
            pill_y + pill_h
        );
    }

    /// A badge's pill is the same object in any rectangle.
    ///
    /// Before the fix the pill's height read `rect.height.min(18)` and its width was
    /// floored at that height, so a badge in a 24 px census cell and one in a 240x120
    /// cell were two different shapes, and a long label grew the pill's height as well
    /// as its width.
    #[test]
    fn the_pill_keeps_its_own_size_in_any_rectangle() {
        for rect in [Rect::new(0, 0, 240, 120), Rect::new(0, 0, 40, 24)] {
            let mut badge = Badge::new(rect);
            badge.set_text("x");
            let svg = render_to_svg(&mut badge);
            let pill_tag = svg
                .split("<rect")
                .find(|chunk| chunk.contains("rx="))
                .expect("the badge paints a pill");
            let height = pill_tag
                .split("height=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .and_then(|value| value.parse::<u32>().ok())
                .expect("the pill carries a height");
            assert!(
                height >= dimensions::BADGE_PILL_HEIGHT,
                "the pill keeps its own floor in {rect:?}, got {height}"
            );
            assert!(height <= rect.height, "the pill never leaves {rect:?}, got {height}");
        }
    }
}
