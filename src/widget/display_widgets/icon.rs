//! Icon widget — renders simple geometric icon representations.
//!
//! The Icon widget displays recognizable geometric shapes for common icon names
//! (Check, Cross, Arrow, Star, Heart, Search, Menu, Close, Plus, Minus, Info,
//! Warning, Error, etc.). Each icon is drawn using basic shapes — lines, circles,
//! rectangles, and paths — through the render context.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Common icon names for use with the Icon widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    Check,
    Cross,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Star,
    Heart,
    Settings,
    Home,
    Search,
    Menu,
    Close,
    Plus,
    Minus,
    Info,
    Warning,
    Error,
    User,
    Mail,
    Bell,
    Edit,
    Trash,
    Share,
    Refresh,
    More,
    Filter,
    Lock,
    Unlock,
    Download,
    Upload,
}

impl IconName {
    /// Returns the string representation of this icon name.
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
pub struct Icon {
    base: BaseWidget,
    icon_name: String,
    size: f32,
    color: Color,
}

impl Icon {
    /// Creates a new Icon widget with the given geometry.
    ///
    /// Defaults to a "check" icon with size 24 and the PRIMARY color.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Icon, geometry, "Icon"),
            icon_name: "check".to_string(),
            size: 24.0,
            color: Color::PRIMARY,
        }
    }

    /// Sets the icon by name. Accepts any string; unrecognized names
    /// render as a simple question mark shape.
    pub fn set_icon(&mut self, name: &str) {
        self.icon_name = name.to_string();
        self.base.request_redraw();
    }

    /// Returns the current icon name string.
    pub fn icon(&self) -> &str {
        &self.icon_name
    }

    /// Sets the icon using an `IconName` enum variant.
    pub fn set_icon_enum(&mut self, icon: IconName) {
        self.set_icon(icon.as_str());
    }

    /// Sets the icon size in logical pixels.
    pub fn set_size(&mut self, size: f32) {
        self.size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the icon size in logical pixels.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// Sets the icon color.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.base.request_redraw();
    }

    /// Returns the icon color.
    pub fn color(&self) -> Color {
        self.color
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
        let c = self.color;
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
}

impl Draw for Icon {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let is_enabled = self.base.is_enabled();
        if !is_enabled {
            let original_color = self.color;
            // Compute grayscale: average of R, G, B channels
            let gray =
                ((original_color.r as u16 + original_color.g as u16 + original_color.b as u16) / 3)
                    as u8;
            // Use muted gray at half alpha for disabled appearance
            self.color = Color::rgba(gray, gray, gray, original_color.a / 2);
            self.draw_icon(context);
            self.color = original_color;
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
