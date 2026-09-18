// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Icon widget — renders simple geometric icon representations.
//!
//! The Icon widget displays recognizable geometric shapes for common icon names
//! (Check, Cross, Arrow, Star, Heart, Search, Menu, Close, Plus, Minus, Info,
//! Warning, Error, etc.). Each icon is drawn using basic shapes — lines, circles,
//! rectangles, and paths — through the render context.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_f64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Common icon names for use with the Icon widget.
///
/// Each variant corresponds to a hand-drawn geometric representation rather
/// than a glyph from an icon font, so the rendered result is a plain-shape
/// approximation of the symbol. Every variant round-trips through
/// [`IconName::as_str`] and [`IconName::from_name`].
///
/// Note that several variants are drawn as aliases of others (for example
/// [`IconName::Close`] and [`IconName::Cross`]), so visually distinct names do
/// not always produce distinct output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    /// A tick, for confirmation or success.
    Check,
    /// Two crossing strokes, for cancel or "no".
    Cross,
    /// A left-pointing arrow, for "back".
    ArrowLeft,
    /// A right-pointing arrow, for "next".
    ArrowRight,
    /// An upward arrow.
    ArrowUp,
    /// A downward arrow.
    ArrowDown,
    /// A five-pointed star, for favourites or ratings.
    Star,
    /// A heart, for likes or favourites.
    Heart,
    /// A gear, for configuration.
    Settings,
    /// A house, for the home view.
    Home,
    /// A magnifying glass, for search.
    Search,
    /// Three stacked bars, for a navigation menu.
    Menu,
    /// The X-shaped dismiss mark.
    Close,
    /// A plus sign, for adding.
    Plus,
    /// A minus sign, for removing.
    Minus,
    /// The letter "i" in a circle, for informational messages.
    Info,
    /// A triangle with an exclamation mark, for warnings.
    Warning,
    /// A circle with an exclamation mark, for errors.
    Error,
    /// A head-and-shoulders silhouette, for an account.
    User,
    /// An envelope, for messages.
    Mail,
    /// A bell, for notifications.
    Bell,
    /// A pencil, for editing.
    Edit,
    /// A waste bin, for deletion.
    Trash,
    /// A node-and-branches glyph, for sharing.
    Share,
    /// A circular arrow, for reloading.
    Refresh,
    /// Three horizontal dots, for an overflow menu.
    More,
    /// A funnel, for filtering.
    Filter,
    /// A closed padlock.
    Lock,
    /// An open padlock.
    Unlock,
    /// A downward arrow into a tray, for downloading.
    Download,
    /// An upward arrow out of a tray, for uploading.
    Upload,
}

impl IconName {
    /// Returns the string representation of this icon name.
    ///
    /// The tokens are lower-case and underscore-separated (`"arrow_left"`),
    /// and are the exact spellings accepted by [`IconName::from_name`] and by
    /// the `icon` property. They are also the names used by
    /// [`Icon::set_icon`].
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Cross => "cross",
            Self::ArrowLeft => "arrow_left",
            Self::ArrowRight => "arrow_right",
            Self::ArrowUp => "arrow_up",
            Self::ArrowDown => "arrow_down",
            Self::Star => "star",
            Self::Heart => "heart",
            Self::Settings => "settings",
            Self::Home => "home",
            Self::Search => "search",
            Self::Menu => "menu",
            Self::Close => "close",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::User => "user",
            Self::Mail => "mail",
            Self::Bell => "bell",
            Self::Edit => "edit",
            Self::Trash => "trash",
            Self::Share => "share",
            Self::Refresh => "refresh",
            Self::More => "more",
            Self::Filter => "filter",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Download => "download",
            Self::Upload => "upload",
        }
    }

    /// Parses an icon name from its string representation.
    ///
    /// The match is exact and case-sensitive: only the tokens produced by
    /// [`IconName::as_str`] are accepted, so `"ArrowLeft"` and `"arrow left"`
    /// both return `None`. Use [`Icon::set_icon`] when an unrecognised name
    /// should fall back to a placeholder rather than being rejected.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "check" => Some(Self::Check),
            "cross" => Some(Self::Cross),
            "arrow_left" => Some(Self::ArrowLeft),
            "arrow_right" => Some(Self::ArrowRight),
            "arrow_up" => Some(Self::ArrowUp),
            "arrow_down" => Some(Self::ArrowDown),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "settings" => Some(Self::Settings),
            "home" => Some(Self::Home),
            "search" => Some(Self::Search),
            "menu" => Some(Self::Menu),
            "close" => Some(Self::Close),
            "plus" => Some(Self::Plus),
            "minus" => Some(Self::Minus),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "user" => Some(Self::User),
            "mail" => Some(Self::Mail),
            "bell" => Some(Self::Bell),
            "edit" => Some(Self::Edit),
            "trash" => Some(Self::Trash),
            "share" => Some(Self::Share),
            "refresh" => Some(Self::Refresh),
            "more" => Some(Self::More),
            "filter" => Some(Self::Filter),
            "lock" => Some(Self::Lock),
            "unlock" => Some(Self::Unlock),
            "download" => Some(Self::Download),
            "upload" => Some(Self::Upload),
            _ => None,
        }
    }
}

/// Icon widget — renders a simple geometric icon.
///
/// The icon is drawn using basic shapes (lines, circles, filled rects) through
/// the render context. The widget supports all common icon names defined in
/// `IconName` and renders a recognizable geometric representation for each.
///
/// # Colour
///
/// The icon's colour is resolved in this order:
///
/// 1. [`Icon::set_color`] / the `color` style property, when either was used.
/// 2. The active theme's resolved text colour for an icon, so an icon follows a
///    light/dark switch along with the text beside it.
/// 3. [`Color::PRIMARY`], the historical default.
///
/// Before step 2 existed the icon was the one themed control that did **not**
/// follow the theme: it hardcoded `Color::PRIMARY` in its constructor, so a dark
/// theme left every icon a bright blue that clashed with the rest of the surface.
///
/// # Sizing and layout
///
/// [`Icon::size`] gives the side of a square bounding box, which is centred in
/// the widget's geometry and clipped to it: if the requested size is larger than
/// the geometry, the icon is drawn at the geometry's smaller dimension and the
/// extra is not scaled down proportionally. A zero-width or zero-height geometry
/// draws nothing.
///
/// # Disabled appearance
///
/// When the widget is disabled, the icon is rendered with a desaturated version
/// of the resolved colour (the mean of its R, G, and B channels) at half the
/// original alpha. The resolved colour is recomputed on the next draw, so the
/// stored value is unaffected.
pub struct Icon {
    base: BaseWidget,
    icon_name: String,
    size: f32,
    /// An explicit colour, if one was set. `None` means "follow the theme".
    color: Option<Color>,
}

impl Icon {
    /// Creates a new Icon widget with the given geometry.
    ///
    /// Defaults to a "check" icon with size 24. The colour is left unset so the
    /// icon follows the active theme; [`Icon::color`] reports the colour that
    /// will actually be used.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Icon, geometry, "Icon"),
            icon_name: "check".to_string(),
            size: 24.0,
            color: None,
        }
    }

    /// Sets the icon by name. Accepts any string; unrecognized names
    /// render as a simple question mark shape.
    ///
    /// The name is stored verbatim and matched case-sensitively at draw time
    /// against [`IconName::as_str`] tokens, so an unknown or differently-cased
    /// name does not fail: it draws the placeholder shape instead. Setting a
    /// name requests a redraw.
    pub fn set_icon(&mut self, name: &str) {
        self.icon_name = name.to_string();
        self.base.request_redraw();
    }

    /// Returns the current icon name string.
    ///
    /// This is whatever was last passed to [`Icon::set_icon`] (or the default
    /// `"check"`), not a canonicalised token: an unrecognised name is returned
    /// as-is even though it draws as the placeholder.
    pub fn icon(&self) -> &str {
        &self.icon_name
    }

    /// Sets the icon using an `IconName` enum variant.
    ///
    /// Equivalent to [`Icon::set_icon`] with the variant's canonical string, so
    /// the name always resolves to a real icon.
    pub fn set_icon_enum(&mut self, icon: IconName) {
        self.set_icon(icon.as_str());
    }

    /// Sets the icon size in logical pixels.
    ///
    /// The size is the length of the icon's square bounding box, not its
    /// stroke or glyph height. Values are clamped to a minimum of `4.0` to keep
    /// the icon legible. Requests a redraw.
    pub fn set_size(&mut self, size: f32) {
        self.size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the icon size in logical pixels.
    ///
    /// This is the requested square bounding-box length, which may exceed the
    /// widget's own extent; see `icon_rect` for how it is fitted into the
    /// geometry.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// Sets an explicit icon colour, overriding the theme.
    ///
    /// The colour is used for every shape in the icon; there is no separate
    /// stroke and fill colour. Requests a redraw.
    pub fn set_color(&mut self, color: Color) {
        self.color = Some(color);
        self.base.request_redraw();
    }

    /// Clears an explicit colour, returning the icon to theme resolution.
    pub fn clear_color(&mut self) {
        self.color = None;
        self.base.request_redraw();
    }

    /// Returns the colour this icon will actually be drawn in.
    ///
    /// Resolves in the order documented on the type: an explicit colour, then the
    /// widget style's text colour (which is what the theme populates), then
    /// [`Color::PRIMARY`].
    pub fn color(&self) -> Color {
        self.resolve_color()
    }

    /// The colour an explicit setter stored, if any.
    ///
    /// Separate from [`Icon::color`] so a caller can tell "not set" from "set to
    /// the same value the theme would have chosen".
    pub fn explicit_color(&self) -> Option<Color> {
        self.color
    }

    /// The colour to draw with, after theme resolution.
    fn resolve_color(&self) -> Color {
        if let Some(explicit) = self.color {
            return explicit;
        }
        if let Some(themed) = self.style().text_color {
            return themed;
        }
        Color::PRIMARY
    }

    // ── Drawing helpers ──

    /// Returns the icon bounding rect centered in the widget geometry.
    fn icon_rect(&self) -> Rect {
        let rect = self.geometry();
        let size = self.size as u32;
        let x = rect.x + ((rect.width as i32) - size as i32) / 2;
        let y = rect.y + ((rect.height as i32) - size as i32) / 2;
        Rect::new(x.max(rect.x), y.max(rect.y), size.min(rect.width), size.min(rect.height))
    }

    /// Draws a check mark (✓).
    fn draw_check(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        ctx.draw_line_stroke(
            Point::new(cx + s / 8, cy + s / 2),
            Point::new(cx + s / 3, cy + s * 3 / 4),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 3, cy + s * 3 / 4),
            Point::new(cx + s * 7 / 8, cy + s / 4),
            c,
            sw,
        );
    }

    /// Draws a cross (✕).
    fn draw_cross(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let pad = s / 5;
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + pad),
            Point::new(cx + s - pad, cy + s - pad),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s - pad, cy + pad),
            Point::new(cx + pad, cy + s - pad),
            c,
            sw,
        );
    }

    /// Draws a left arrow (←).
    fn draw_arrow_left(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let mid_y = cy + s / 2;
        let tip_x = cx + s / 5;
        ctx.draw_line_stroke(Point::new(cx + s * 4 / 5, mid_y), Point::new(tip_x, mid_y), c, sw);
        ctx.draw_line_stroke(Point::new(tip_x, mid_y), Point::new(cx + s / 3, cy + s / 4), c, sw);
        ctx.draw_line_stroke(
            Point::new(tip_x, mid_y),
            Point::new(cx + s / 3, cy + s * 3 / 4),
            c,
            sw,
        );
    }

    /// Draws a right arrow (→).
    fn draw_arrow_right(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let mid_y = cy + s / 2;
        let tip_x = cx + s * 4 / 5;
        ctx.draw_line_stroke(Point::new(cx + s / 5, mid_y), Point::new(tip_x, mid_y), c, sw);
        ctx.draw_line_stroke(
            Point::new(tip_x, mid_y),
            Point::new(cx + s * 2 / 3, cy + s / 4),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(tip_x, mid_y),
            Point::new(cx + s * 2 / 3, cy + s * 3 / 4),
            c,
            sw,
        );
    }

    /// Draws an up arrow (↑).
    fn draw_arrow_up(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let mid_x = cx + s / 2;
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 4 / 5),
            Point::new(mid_x, cy + s / 5),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 5),
            Point::new(cx + s / 4, cy + s / 3),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 5),
            Point::new(cx + s * 3 / 4, cy + s / 3),
            c,
            sw,
        );
    }

    /// Draws a down arrow (↓).
    fn draw_arrow_down(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let mid_x = cx + s / 2;
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 5),
            Point::new(mid_x, cy + s * 4 / 5),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 4 / 5),
            Point::new(cx + s / 4, cy + s * 2 / 3),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 4 / 5),
            Point::new(cx + s * 3 / 4, cy + s * 2 / 3),
            c,
            sw,
        );
    }

    /// Draws a star (★).
    fn draw_star(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let half = s / 2;
        let center = Point::new(cx + half, cy + half);
        // Draw a star using overlapping lines from center
        let arm_len = half;
        for i in 0..5 {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * 2.0 * std::f32::consts::PI / 5.0;
            let outer_x = center.x + (arm_len as f32 * angle.cos()) as i32;
            let outer_y = center.y + (arm_len as f32 * angle.sin()) as i32;
            ctx.draw_line_stroke(center, Point::new(outer_x, outer_y), c, 2);
        }
    }

    /// Draws a heart (♥).
    fn draw_heart(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let half = s / 2;
        let center = Point::new(cx + half, cy + half);
        let lobe_r = (s / 4).max(2) as u32;
        // Two lobes at top
        ctx.fill_circle(Point::new(center.x - s / 5, center.y - s / 6), lobe_r, c);
        ctx.fill_circle(Point::new(center.x + s / 5, center.y - s / 6), lobe_r, c);
        // Triangle body pointing down
        ctx.draw_line_stroke(
            Point::new(center.x - s / 3, center.y - s / 8),
            Point::new(center.x, center.y + s / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(center.x + s / 3, center.y - s / 8),
            Point::new(center.x, center.y + s / 3),
            c,
            2,
        );
    }

    /// Draws a search/magnifying glass icon.
    fn draw_search(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let center = Point::new(cx + s / 3, cy + s / 3);
        let circle_r = (s / 5).max(2) as u32;
        ctx.draw_circle_stroke(center, circle_r, c, 2);
        // Handle
        let handle_start = Point::new(
            center.x + (circle_r as f32 * 0.7) as i32,
            center.y + (circle_r as f32 * 0.7) as i32,
        );
        ctx.draw_line_stroke(handle_start, Point::new(cx + s * 4 / 5, cy + s * 4 / 5), c, 2);
    }

    /// Draws a menu/hamburger icon (≡).
    fn draw_menu(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let sw = (self.size / 12.0).max(1.5) as u32;
        let pad = s / 5;
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + s / 4),
            Point::new(cx + s - pad, cy + s / 4),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + s / 2),
            Point::new(cx + s - pad, cy + s / 2),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + s * 3 / 4),
            Point::new(cx + s - pad, cy + s * 3 / 4),
            c,
            sw,
        );
    }

    /// Draws a close/X icon (same as cross).
    fn draw_close(&self, ctx: &mut RenderContext) {
        // Reuse cross drawing
        self.draw_cross(ctx);
    }

    /// Draws a plus (+) icon.
    fn draw_plus(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let pad = s / 4;
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + s / 2),
            Point::new(cx + s - pad, cy + s / 2),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + pad),
            Point::new(cx + s / 2, cy + s - pad),
            c,
            sw,
        );
    }

    /// Draws a minus (−) icon.
    fn draw_minus(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let sw = (self.size / 12.0).max(1.5) as u32;
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let pad = s / 4;
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + s / 2),
            Point::new(cx + s - pad, cy + s / 2),
            c,
            sw,
        );
    }

    /// Draws an info (i) icon.
    fn draw_info(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let center = Point::new(cx + s / 2, cy + s / 2);
        // Circle
        ctx.draw_circle_stroke(center, (s / 2 - 1).max(2) as u32, c, 2);
        // Dot at top
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 4), 2, c);
        // Vertical line for body
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 3 + 2),
            Point::new(cx + s / 2, cy + s * 3 / 4),
            c,
            2,
        );
    }

    /// Draws a warning (!) icon.
    fn draw_warning(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Triangle outline
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 6),
            Point::new(cx + s / 6, cy + s * 5 / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 6),
            Point::new(cx + s * 5 / 6, cy + s * 5 / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s * 5 / 6),
            Point::new(cx + s * 5 / 6, cy + s * 5 / 6),
            c,
            2,
        );
        // Exclamation mark
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 3),
            Point::new(cx + s / 2, cy + s * 2 / 3),
            c,
            2,
        );
        // Dot at bottom
        ctx.fill_circle(Point::new(cx + s / 2, cy + s * 3 / 4), 2, c);
    }

    /// Draws an error (✕ in circle) icon.
    fn draw_error(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let center = Point::new(cx + s / 2, cy + s / 2);
        // Circle outline
        ctx.draw_circle_stroke(center, (s / 2 - 1).max(2) as u32, c, 2);
        // X inside
        let pad = s / 4;
        let sw = 2_u32;
        ctx.draw_line_stroke(
            Point::new(cx + pad, cy + pad),
            Point::new(cx + s - pad, cy + s - pad),
            c,
            sw,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s - pad, cy + pad),
            Point::new(cx + pad, cy + s - pad),
            c,
            sw,
        );
    }

    /// Dispatches to the correct draw method based on icon_name.
    fn draw_icon(&self, ctx: &mut RenderContext) {
        match self.icon_name.as_str() {
            "check" => self.draw_check(ctx),
            "cross" => self.draw_cross(ctx),
            "close" => self.draw_close(ctx),
            "arrow_left" => self.draw_arrow_left(ctx),
            "arrow_right" => self.draw_arrow_right(ctx),
            "arrow_up" => self.draw_arrow_up(ctx),
            "arrow_down" => self.draw_arrow_down(ctx),
            "star" => self.draw_star(ctx),
            "heart" => self.draw_heart(ctx),
            "search" => self.draw_search(ctx),
            "menu" => self.draw_menu(ctx),
            "plus" => self.draw_plus(ctx),
            "minus" => self.draw_minus(ctx),
            "info" => self.draw_info(ctx),
            "warning" => self.draw_warning(ctx),
            "error" => self.draw_error(ctx),
            // For remaining icons, draw simple representations
            "settings" => self.draw_settings(ctx),
            "home" => self.draw_home(ctx),
            "user" => self.draw_user(ctx),
            "mail" => self.draw_mail(ctx),
            "bell" => self.draw_bell(ctx),
            "edit" => self.draw_edit(ctx),
            "trash" => self.draw_trash(ctx),
            "share" => self.draw_share(ctx),
            "refresh" => self.draw_refresh(ctx),
            "more" => self.draw_more(ctx),
            "filter" => self.draw_filter(ctx),
            "lock" => self.draw_lock(ctx),
            "unlock" => self.draw_unlock(ctx),
            "download" => self.draw_download(ctx),
            "upload" => self.draw_upload(ctx),
            _ => self.draw_unknown(ctx),
        }
    }

    // ── Extended icon drawings ──

    fn draw_settings(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let center = Point::new(cx + s / 2, cy + s / 2);
        // Gear: circle with spokes
        ctx.draw_circle_stroke(center, (s / 3).max(2) as u32, c, 2);
        for i in 0..6 {
            let angle = i as f32 * std::f32::consts::PI / 3.0;
            let inner = Point::new(
                center.x + ((s / 4) as f32 * angle.cos()) as i32,
                center.y + ((s / 4) as f32 * angle.sin()) as i32,
            );
            let outer = Point::new(
                center.x + ((s / 2 - 1) as f32 * angle.cos()) as i32,
                center.y + ((s / 2 - 1) as f32 * angle.sin()) as i32,
            );
            ctx.draw_line_stroke(inner, outer, c, 2);
        }
    }

    fn draw_home(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // House shape: roof + walls
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 6),
            Point::new(cx + s / 6, cy + s / 2),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 6),
            Point::new(cx + s * 5 / 6, cy + s / 2),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s / 2),
            Point::new(cx + s / 6, cy + s * 5 / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 5 / 6, cy + s / 2),
            Point::new(cx + s * 5 / 6, cy + s * 5 / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s * 5 / 6),
            Point::new(cx + s * 5 / 6, cy + s * 5 / 6),
            c,
            2,
        );
        // Door
        ctx.draw_line_stroke(
            Point::new(cx + s / 3, cy + s * 5 / 6),
            Point::new(cx + s / 3, cy + s * 2 / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 2 / 3, cy + s * 5 / 6),
            Point::new(cx + s * 2 / 3, cy + s * 2 / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 3, cy + s * 2 / 3),
            Point::new(cx + s * 2 / 3, cy + s * 2 / 3),
            c,
            2,
        );
    }

    fn draw_user(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Head circle
        ctx.draw_circle_stroke(
            Point::new(cx + s / 2, cy + s / 3 - s / 8),
            (s / 6).max(2) as u32,
            c,
            2,
        );
        // Body
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 3 + s / 8),
            Point::new(cx + s / 2, cy + s * 3 / 4),
            c,
            2,
        );
        // Arms
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 2),
            Point::new(cx + s * 3 / 4, cy + s / 2),
            c,
            2,
        );
        // Legs
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s * 3 / 4),
            Point::new(cx + s / 4, cy + s * 5 / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s * 3 / 4),
            Point::new(cx + s * 3 / 4, cy + s * 5 / 6),
            c,
            2,
        );
    }

    fn draw_mail(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Envelope
        ctx.draw_rect_stroke(
            Rect::new(cx + s / 6, cy + s / 4, (s * 2 / 3) as u32, (s / 2) as u32),
            c,
            2,
        );
        // Flap
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s / 4),
            Point::new(cx + s / 2, cy + s / 2),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 2),
            Point::new(cx + s * 5 / 6, cy + s / 4),
            c,
            2,
        );
    }

    fn draw_bell(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Bell body (arc + bottom)
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 2),
            Point::new(cx + s / 4, cy + s / 4),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 2),
            Point::new(cx + s * 3 / 4, cy + s / 4),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 4),
            Point::new(cx + s / 4, cy + s / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 4),
            Point::new(cx + s * 3 / 4, cy + s / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 2),
            Point::new(cx + s * 3 / 4, cy + s / 2),
            c,
            2,
        );
        // Top arc
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 6),
            Point::new(cx + s / 2, cy + s / 8),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 6),
            Point::new(cx + s / 2, cy + s / 8),
            c,
            2,
        );
        // Clapper
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 2), 2, c);
    }

    fn draw_edit(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Pencil
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s * 3 / 4),
            Point::new(cx + s * 3 / 4, cy + s / 4),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 4),
            Point::new(cx + s * 5 / 6, cy + s / 3),
            c,
            2,
        );
        // Tip
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s * 3 / 4),
            Point::new(cx + s / 6, cy + s * 5 / 6),
            c,
            2,
        );
    }

    fn draw_trash(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Bin outline
        ctx.draw_rect_stroke(
            Rect::new(cx + s / 4, cy + s / 3, (s / 2) as u32, (s / 2) as u32),
            c,
            2,
        );
        // Lid
        ctx.draw_line_stroke(
            Point::new(cx + s / 5, cy + s / 3),
            Point::new(cx + s * 4 / 5, cy + s / 3),
            c,
            2,
        );
        // Handle
        ctx.draw_line_stroke(
            Point::new(cx + s / 3, cy + s / 6),
            Point::new(cx + s * 2 / 3, cy + s / 6),
            c,
            2,
        );
        // Lines inside
        ctx.draw_line_stroke(
            Point::new(cx + s / 3, cy + s / 2),
            Point::new(cx + s / 3, cy + s * 2 / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 2),
            Point::new(cx + s / 2, cy + s * 2 / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 2 / 3, cy + s / 2),
            Point::new(cx + s * 2 / 3, cy + s * 2 / 3),
            c,
            2,
        );
    }

    fn draw_share(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Share: three dots connected by lines
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 5), 2, c);
        ctx.fill_circle(Point::new(cx + s / 5, cy + s * 3 / 4), 2, c);
        ctx.fill_circle(Point::new(cx + s * 4 / 5, cy + s * 3 / 4), 2, c);
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 5),
            Point::new(cx + s / 5, cy + s * 3 / 4),
            c,
            1,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s / 5),
            Point::new(cx + s * 4 / 5, cy + s * 3 / 4),
            c,
            1,
        );
    }

    fn draw_refresh(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        let center = Point::new(cx + s / 2, cy + s / 2);
        // Circle
        ctx.draw_circle_stroke(center, (s / 3).max(2) as u32, c, 2);
        // Arrow head
        ctx.draw_line_stroke(
            Point::new(cx + s * 4 / 5, cy + s / 4),
            Point::new(cx + s * 4 / 5, cy + s / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 4 / 5, cy + s / 4),
            Point::new(cx + s * 3 / 4, cy + s / 4),
            c,
            2,
        );
    }

    fn draw_more(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Three vertical dots
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 4), 2, c);
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 2), 2, c);
        ctx.fill_circle(Point::new(cx + s / 2, cy + s * 3 / 4), 2, c);
    }

    fn draw_filter(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Funnel
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s / 6),
            Point::new(cx + s * 5 / 6, cy + s / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 5 / 6, cy + s / 6),
            Point::new(cx + s / 2, cy + s * 2 / 3),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 6, cy + s / 6),
            Point::new(cx + s / 2, cy + s * 2 / 3),
            c,
            2,
        );
        // Stem
        ctx.draw_line_stroke(
            Point::new(cx + s / 2, cy + s * 2 / 3),
            Point::new(cx + s / 2, cy + s * 5 / 6),
            c,
            2,
        );
    }

    fn draw_lock(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Shackle
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 3),
            Point::new(cx + s / 4, cy + s / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 3),
            Point::new(cx + s * 3 / 4, cy + s / 6),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 6),
            Point::new(cx + s * 3 / 4, cy + s / 6),
            c,
            2,
        );
        // Lock body
        ctx.draw_rect_stroke(
            Rect::new(cx + s / 4, cy + s / 3, (s / 2) as u32, (s / 2) as u32),
            c,
            2,
        );
        // Keyhole
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 2 + s / 8), 2, c);
    }

    fn draw_unlock(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Open shackle (only right side)
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s / 3),
            Point::new(cx + s / 4, cy + s / 8),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(cx + s * 3 / 4, cy + s / 3),
            Point::new(cx + s * 3 / 4, cy + s / 6),
            c,
            2,
        );
        // Lock body
        ctx.draw_rect_stroke(
            Rect::new(cx + s / 4, cy + s / 3, (s / 2) as u32, (s / 2) as u32),
            c,
            2,
        );
        // Keyhole
        ctx.fill_circle(Point::new(cx + s / 2, cy + s / 2 + s / 8), 2, c);
    }

    fn draw_download(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Arrow down
        let mid_x = cx + s / 2;
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 4),
            Point::new(mid_x, cy + s * 3 / 4),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 3 / 4),
            Point::new(cx + s / 3, cy + s / 2),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 3 / 4),
            Point::new(cx + s * 2 / 3, cy + s / 2),
            c,
            2,
        );
        // Base line
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s * 4 / 5),
            Point::new(cx + s * 3 / 4, cy + s * 4 / 5),
            c,
            2,
        );
    }

    fn draw_upload(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        // Arrow up
        let mid_x = cx + s / 2;
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s * 3 / 4),
            Point::new(mid_x, cy + s / 4),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 4),
            Point::new(cx + s / 3, cy + s / 2),
            c,
            2,
        );
        ctx.draw_line_stroke(
            Point::new(mid_x, cy + s / 4),
            Point::new(cx + s * 2 / 3, cy + s / 2),
            c,
            2,
        );
        // Base line
        ctx.draw_line_stroke(
            Point::new(cx + s / 4, cy + s * 4 / 5),
            Point::new(cx + s * 3 / 4, cy + s * 4 / 5),
            c,
            2,
        );
    }

    /// Draws an unknown icon as a question mark.
    fn draw_unknown(&self, ctx: &mut RenderContext) {
        let r = self.icon_rect();
        let c = self.resolve_color();
        let cx = r.x;
        let cy = r.y;
        let s = r.width as i32;
        ctx.draw_text(
            Point::new(cx + s / 4, cy + s * 3 / 4),
            "?",
            &crate::core::Font::simple("sans-serif", 12.0),
            c,
            HorizontalAlignment::Left,
        );
    }
}

impl Widget for Icon {
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

/// `Icon`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for Icon {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "icon_name" => Ok(CapabilityValue::String(self.icon().to_string())),
            "size" => Ok(CapabilityValue::Float(f64::from(self.size()))),
            // Reports the *resolved* colour, which is what a reader wants: the
            // colour a draw would use, whether that came from a setter or the
            // theme.
            "color" => Ok(CapabilityValue::String(self.resolve_color().to_hex_rgba())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "icon_name" => {
                self.set_icon(&expect_string(value)?);
                Ok(())
            }
            "size" => {
                self.set_size(expect_f64(value)? as f32);
                Ok(())
            }
            "color" => {
                let raw = expect_string(value)?;
                let color = Color::parse_hex(&raw).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_color(color);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["icon_name", "size", "color", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `icon` publishes.
    ///
    /// Both published names assign state: `set_icon_name` a name and `set_size` a
    /// pixel size. Neither has a zero-argument meaning in this control's API, so both
    /// are refused as [`CapabilityAccessError::OutOfRange`] — use `set("icon_name", ..)`
    /// / `set("size", ..)` — rather than reported as unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_icon_name" | "set_size" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Icon {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        if !self.base.is_enabled() {
            // Render the disabled appearance by temporarily pinning an explicit
            // colour: a desaturated grey at half the resolved colour's alpha. The
            // pin is removed afterwards, so the pre-draw resolution (theme or
            // explicit setter) is unchanged and the next draw recomputes it.
            let resolved = self.resolve_color();
            let gray = ((resolved.r as u16 + resolved.g as u16 + resolved.b as u16) / 3) as u8;
            self.color = Some(Color::rgba(gray, gray, gray, resolved.a / 2));
            self.draw_icon(context);
            self.color = None;
            return;
        }
        self.draw_icon(context);
    }
}

impl EventHandler for Icon {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_default_creation() {
        let icon = Icon::new(Rect::new(0, 0, 24, 24));
        assert_eq!(icon.kind(), WidgetKind::Icon);
        assert_eq!(icon.icon(), "check");
        assert!((icon.size() - 24.0).abs() < f32::EPSILON);
        assert_eq!(icon.color(), Color::PRIMARY);
    }

    #[test]
    fn icon_set_icon_and_color() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("heart");
        assert_eq!(icon.icon(), "heart");

        icon.set_icon_enum(IconName::Star);
        assert_eq!(icon.icon(), "star");

        icon.set_color(Color::RED);
        assert_eq!(icon.color(), Color::RED);

        icon.set_size(32.0);
        assert!((icon.size() - 32.0).abs() < f32::EPSILON);
    }

    // ── Theme-driven colour ──────────────────────────────────────────────

    /// A freshly created icon reports the fallback colour, and an explicit setter
    /// still wins over it.
    #[test]
    fn colour_resolution_prefers_an_explicit_setter() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        assert_eq!(icon.explicit_color(), None, "a new icon follows the theme");
        assert_eq!(icon.color(), Color::PRIMARY, "with no theme, the fallback stands");

        icon.set_color(Color::RED);
        assert_eq!(icon.explicit_color(), Some(Color::RED));
        assert_eq!(icon.color(), Color::RED);

        icon.clear_color();
        assert_eq!(icon.explicit_color(), None, "clearing returns the icon to the theme");
        assert_eq!(icon.color(), Color::PRIMARY);
    }

    /// The icon picks up the text colour from its `WidgetStyle`, which is what the
    /// theme populates. Before this the icon ignored `WidgetStyle` entirely and
    /// stayed `Color::PRIMARY` through a theme switch.
    #[test]
    fn colour_resolution_follows_the_widget_style() {
        use crate::style::WidgetStyle;

        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        let themed = Color::rgba(7, 8, 9, 255);
        icon.set_style(WidgetStyle::default().with_text_color(themed));

        assert_eq!(icon.color(), themed, "the style's text colour must drive the icon");

        // An explicit colour still outranks the style.
        icon.set_color(Color::RED);
        assert_eq!(icon.color(), Color::RED);
    }

    /// The `color` property is readable and writable, and a malformed value is
    /// rejected rather than silently ignored.
    #[test]
    fn the_color_property_round_trips_and_rejects_garbage() {
        use crate::widget::capability::WidgetProperties;

        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        assert!(icon.property_names().contains(&"color"));

        icon.set("color", CapabilityValue::String("#11223344".to_string()))
            .expect("a hex colour must be accepted");
        assert_eq!(icon.color(), Color::rgba(0x11, 0x22, 0x33, 0x44));

        assert_eq!(
            icon.set("color", CapabilityValue::String("not-a-colour".to_string())),
            Err(CapabilityAccessError::TypeMismatch),
            "an unparsable colour must be reported"
        );
    }

    #[test]
    fn icon_name_enum_roundtrip() {
        for name in &[
            IconName::Check,
            IconName::Cross,
            IconName::ArrowLeft,
            IconName::ArrowRight,
            IconName::ArrowUp,
            IconName::ArrowDown,
            IconName::Star,
            IconName::Heart,
            IconName::Search,
            IconName::Menu,
            IconName::Close,
            IconName::Plus,
            IconName::Minus,
            IconName::Info,
            IconName::Warning,
            IconName::Error,
            IconName::Home,
            IconName::Settings,
            IconName::User,
            IconName::Mail,
            IconName::Bell,
            IconName::Edit,
            IconName::Trash,
            IconName::Share,
            IconName::Refresh,
            IconName::More,
            IconName::Filter,
            IconName::Lock,
            IconName::Unlock,
            IconName::Download,
            IconName::Upload,
        ] {
            let s = name.as_str();
            let parsed = IconName::from_name(s);
            assert!(parsed.is_some(), "Failed to parse icon name: {s}");
            assert_eq!(parsed.unwrap(), *name);
        }
    }

    #[test]
    fn icon_svg_output_check() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("check");

        let svg = crate::widget::svg::render_to_svg(&mut icon);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn icon_svg_output_star() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon_enum(IconName::Star);

        let svg = crate::widget::svg::render_to_svg(&mut icon);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn icon_svg_output_warning() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("warning");

        let svg = crate::widget::svg::render_to_svg(&mut icon);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn icon_event_forwarding() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        // Should not panic
        icon.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        icon.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }

    #[test]
    fn icon_unknown_fallback() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("nonexistent");

        let svg = crate::widget::svg::render_to_svg(&mut icon);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
