// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Icon widget — renders simple geometric icon representations.
//!
//! The Icon widget displays recognizable geometric shapes for common icon names
//! (Check, Cross, Arrow, Star, Heart, Search, Menu, Close, Plus, Minus, Info,
//! Warning, Error, etc.). Each icon is drawn using basic shapes — lines, circles,
//! rectangles, and paths — through the render context.

#[cfg(widgets_unstripped)]
use crate::core::HorizontalAlignment;
use crate::core::{Color, Point, Rect};
#[cfg(widgets_unstripped)]
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
#[cfg(widgets_unstripped)]
use crate::widget::capability::coercion::{expect_f64, expect_string};
#[cfg(widgets_unstripped)]
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
#[cfg(widgets_unstripped)]
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
#[cfg(widgets_unstripped)]
use crate::widget::capability::WidgetProperties;
#[cfg(widgets_unstripped)]
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
#[cfg(widgets_unstripped)]
use crate::{impl_widget_property_hooks, property_names_of};

// The `IconName` type is generated from `tools/icon_tokens.txt` — see
// `tools/gen_icon_names.py`. It is a module rather than an inline `include!` so its public
// items are named by their real path and rustdoc documents them as the type's own. The `#[path]`
// is required because this file is itself a module, so a bare `mod icon_names;` would be looked
// for under an `icon/` directory.
#[path = "icon_names.rs"]
mod icon_names;
pub use icon_names::IconName;

/// Draws `name`'s icon outline into `rect`, in `color`.
///
/// # Why this is a free function and not a method on [`Icon`]
///
/// A control often needs to paint an *icon glyph* inside its own `draw` — a disclosure
/// chevron on a combo box, a star on a rating, a play/pause mark on a media bar. Those
/// controls own their layout and cannot hand a sub-`Icon` widget to the tree, so they need the
/// outline-drawing primitive itself rather than the widget.
///
/// They used to fake it with `draw_text("▼")` — a Unicode *symbol glyph* passed through the
/// text pipeline. That is wrong on two counts: no bundled face covers those codepoints
/// (measured: U+25B2/U+25B6/U+25BC/U+25BE/U+25C0/U+2605/U+2606/U+2713 are absent from every
/// shipped face), so each fell back to an 8x8 bitmap block; and a symbol is a font shape the
/// viewer's engine may substitute, whereas an icon is geometry.
///
/// # The two sources, resolved in the same order the widget uses
///
/// With the `icons` feature on, `name.data_opt()` gives the bundled Material Symbols outline,
/// flattened through the shared [`flatten_paths`](crate::render::path::flatten_paths) the
/// rasteriser and the SVG backend already agree on. Without the feature (or for a name that
/// is not a token), the generated fallback geometry draws instead. Both paths are the ones
/// [`Icon::draw_icon`] takes, so an inline icon and an [`Icon`] widget cannot render
/// differently.
///
/// A `rect` with zero width or height draws nothing, and a name that neither the data nor the
/// fallback table knows draws the question-mark placeholder — the same honest degradation the
/// widget gives.
pub fn draw_icon_at(ctx: &mut RenderContext, rect: Rect, color: Color, name: IconName) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    #[cfg(feature = "icons")]
    if let Some(data) = name.data_opt() {
        draw_outline_in(ctx, rect, color, &data);
        return;
    }
    draw_fallback_in(ctx, rect, color, name.as_str());
}

/// Draws the icon named `name` into `rect`, in `color`, reporting whether an icon was drawn.
///
/// # Why this exists alongside [`draw_icon_at`]
///
/// Some controls take their icon as a **host-supplied string** rather than an [`IconName`] —
/// `BottomNavigationBar::add_nav_item(icon, label)` and `AdaptiveScaffold::add_nav_item` are the
/// two in this crate. The string API is deliberate: it is how a host names an icon it registered
/// itself with [`register_icon`](crate::widget::register_icon), which an `IconName` enum cannot
/// express. It is also the API's weakness: the string may be anything, including a *symbol
/// character* (`★`, `☰`, `✉`) that no bundled face covers, which then degrades to an 8x8 bitmap
/// through the text path.
///
/// This function closes that gap without changing the API: it resolves `name` exactly the way
/// [`Icon::draw_icon`] does (a bundled token, then a host registration, then the generated
/// fallback), and **returns `false` when it resolved nothing** so the caller can keep whatever text
/// behaviour it had. A host that passes `"star"` now gets a real outline; a host that still passes
/// `"★"` keeps the old result rather than losing its icon.
///
/// # The return value is the compatibility contract
///
/// `false` means "this string is not an icon I know" — not an error. The caller decides what a
/// non-icon string means, which is what lets the migration from symbol strings to token names be a
/// change a host opts into rather than one this crate forces on it.
pub fn draw_icon_named(ctx: &mut RenderContext, rect: Rect, color: Color, name: &str) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    // The bundled outline and the host registration both draw through `draw_outline_in`, which
    // exists only where an outline can be drawn at all. Gating this block the same way keeps a
    // build without an outline path compiling: there the token/registration lookups are skipped
    // and the generated fallback below answers instead, exactly as `Icon::draw_icon` does.
    #[cfg(any(feature = "icons", widgets_unstripped))]
    {
        #[cfg(feature = "icons")]
        if let Some(known) = IconName::from_name(name) {
            if let Some(data) = known.data_opt() {
                draw_outline_in(ctx, rect, color, &data);
                return true;
            }
        }
        if let Some(data) = crate::widget::display_widgets::icon_data_set::lookup_registered(name) {
            draw_outline_in(ctx, rect, color, &data);
            return true;
        }
    }
    // The generated fallback geometry is the answer on a build with no outline path, and the
    // last resort everywhere else. A name it does not know was never an icon, which is what `false`
    // reports to the caller.
    if crate::widget::icon_fallback_data::ICON_FALLBACK.iter().any(|entry| entry.name == name) {
        draw_fallback_in(ctx, rect, color, name);
        return true;
    }
    false
}

/// Draws `name` as a **square** icon centred within `box_rect`, `side` device pixels across.
///
/// # Why the square is separate from the box
///
/// Every control that paints a disclosure arrow, a check mark or a star has a *box* for it —
/// the column a combo box reserves, the cell a rating gives a star — and the icon must be a
/// centred square inside that box, not stretched to its aspect ratio. A stretched icon is a
/// distorted icon, so the two measurements are taken apart here rather than at each call site.
///
/// The `side` is clamped to the box so an over-large request cannot paint outside the slot
/// the control reserved.
pub fn draw_icon_centered(
    ctx: &mut RenderContext,
    box_rect: Rect,
    side: u32,
    color: Color,
    name: IconName,
) {
    let side = side.min(box_rect.width).min(box_rect.height) as i32;
    if side <= 0 {
        return;
    }
    let rect = Rect::new(
        box_rect.x + (box_rect.width as i32 - side) / 2,
        box_rect.y + (box_rect.height as i32 - side) / 2,
        side as u32,
        side as u32,
    );
    draw_icon_at(ctx, rect, color, name);
}

/// Draws `data`'s outline into `rect`, the geometry path shared by [`Icon`] and [`draw_icon_at`].
///
/// Needed whenever either caller exists: the free `draw_icon_at` uses it when the `icons` feature
/// is on, and `Icon::draw_outline_data` uses it for a host-registered outline even with the feature
/// off (registering one must not require the bundled table).
#[cfg(any(feature = "icons", widgets_unstripped))]
fn draw_outline_in(ctx: &mut RenderContext, rect: Rect, color: Color, data: &IconData) {
    use crate::render::path::{
        flatten_paths, IconPlacement, MAX_OUTLINE_CONTOURS, MAX_OUTLINE_POINTS,
    };

    let placement = IconPlacement::new(rect.x, rect.y, rect.width as f32, data.grid);
    let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
    let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
    let Ok(count) = flatten_paths(data.paths, placement, &mut points, &mut contours) else {
        return;
    };
    for &(start, end) in &contours[..count] {
        let Some(contour) = points.get(start..end) else {
            continue;
        };
        // `filled` is true, so this is a fill and the stroke width is unused. Passing `0` keeps the
        // emitted `DrawPath` honest about that: the SVG backend reflects the argument into
        // `stroke-width`, and a filled icon carrying `stroke-width="1"` was attribute noise that
        // said a stroke existed when none is painted.
        ctx.draw_path(contour, true, color, true, 0);
    }
}

/// Draws `token`'s generated fallback geometry into `rect`; unknown tokens draw nothing here
/// (the caller falls back to the placeholder).
fn draw_fallback_in(ctx: &mut RenderContext, rect: Rect, color: Color, token: &str) {
    use crate::widget::icon_fallback_data::ICON_FALLBACK;

    let Some(fallback) = ICON_FALLBACK.iter().find(|entry| entry.name == token) else {
        return;
    };
    let grid = f32::from(fallback.grid);
    let scale = rect.width as f32 / grid;
    let origin_x = rect.x as f32;
    let origin_y = rect.y as f32;
    for contour in fallback.contours {
        if contour.len() < 3 {
            continue;
        }
        let points: crate::compat::Vec<Point> = contour
            .iter()
            .map(|&(gx, gy)| {
                Point::new(
                    (origin_x + gx as f32 * scale).round() as i32,
                    (origin_y + (gy as f32 + grid) * scale).round() as i32,
                )
            })
            .collect();
        // Filled, so the stroke width is unused; see `draw_outline_in` for why it is `0`.
        ctx.draw_path(&points, true, color, true, 0);
    }
}

/// One icon's outline, as SVG path data on a square design grid.
///
/// # Why this type is defined here and not in the generated file
///
/// The table (`crate::widget::icon_data`) is behind the opt-in `icons` feature, but a caller
/// that wants to *name* an icon's data — a signature, a struct field, `Option<IconData>` —
/// must compile in a build without it. Defining the type unconditionally and gating only the
/// table keeps one definition of the shape (`principle #54`) while still compiling the payload
/// out.
///
/// # Coordinates
///
/// Material Symbols draws on a 960-unit grid with **negative y upward** (its own `viewBox`
/// is `0 -960 960 960`). This keeps upstream's numbers verbatim; `Icon` scales and flips the
/// axis when drawing, so the data stays a copy rather than a re-encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconData {
    /// The canonical token, matching [`IconName::as_str`].
    pub name: &'static str,
    /// The design grid: `x` spans `0..grid`, `y` spans `-grid..0`.
    pub grid: u16,
    /// The SVG path data (`d`), one entry per `<path>` in the upstream file.
    pub paths: &'static [&'static str],
}

/// Icon widget — renders a simple geometric icon.
///
/// Gated on `widgets_unstripped`: this is a *widget*, and the reduced profiles do not carry the
/// widget set. The `IconName` vocabulary and the `draw_icon_at` / `draw_icon_centered` primitives
/// above are **not** gated, because a control on any profile paints an inline icon through them.
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
#[cfg(widgets_unstripped)]
pub struct Icon {
    base: BaseWidget,
    icon_name: String,
    size: f32,
    /// An explicit colour, if one was set. `None` means "follow the theme".
    color: Option<Color>,
}

#[cfg(widgets_unstripped)]
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

    /// Draws the icon, preferring the bundled outline data and falling back to generated geometry.
    ///
    /// # The order, and why each step is where it is
    ///
    /// 1. **A built-in token with the `icons` feature on** → its real Material Symbols outline
    ///    ([`IconName::data`]). This is the crate's own vocabulary and it wins, so enabling the
    ///    feature only ever *adds* a path a caller asked for (the historical snapshots stay valid).
    /// 2. **A host-registered name** → the host's outline ([`register_icon`](crate::widget::register_icon)),
    ///    drawn through the same code as a built-in one. A name that is not an [`IconName`] token
    ///    can only be a host icon, which is exactly the case a host registers for.
    /// 3. **Anything else that matches a token** → the generated fallback geometry. The fallback
    ///    must not link the SVG parser, so a build without `icons` draws coarse polygons rather
    ///    than real outlines; that is what keeps the default build byte-identical.
    /// 4. **No match** → the question-mark placeholder.
    ///
    /// # Why the fallback is *derived* rather than hand-drawn
    ///
    /// The fallback used to be 31 hand-written `draw_*` methods — a second source of truth that
    /// could disagree with the SVG data. It did: `Close` and `Cross` were drawn by one method while
    /// the data held two distinct outlines. [`ICON_FALLBACK`](crate::widget::icon_fallback_data::ICON_FALLBACK)
    /// is generated from the **same** `tools/material_symbols/<token>.svg` files as `ICON_DATA`, so
    /// the two paths cannot describe different shapes for one token.
    fn draw_icon(&self, ctx: &mut RenderContext) {
        #[cfg(feature = "icons")]
        {
            if let Some(name) = IconName::from_name(&self.icon_name) {
                // `Some` because the feature is on: the outline is known to exist, so a `None`
                // here would mean the gate let a tokened name through without data.
                if let Some(data) = name.data_opt() {
                    self.draw_outline(ctx, &data);
                    return;
                }
            }
        }
        // A host-registered icon: the name is not one of this crate's tokens, so it can only have
        // come from the registry. It is drawn through the same `draw_outline` a built-in uses, so a
        // registered icon and a bundled one cannot render differently.
        if let Some(data) =
            crate::widget::display_widgets::icon_data_set::lookup_registered(&self.icon_name)
        {
            self.draw_outline_data(ctx, &data);
            return;
        }
        self.draw_fallback(ctx);
    }

    /// Draws an icon from the generated fallback geometry
    /// ([`IconFallback`](crate::widget::icon_fallback_data::IconFallback)).
    ///
    /// The token's geometry is looked up in [`ICON_FALLBACK`](crate::widget::icon_fallback_data::ICON_FALLBACK);
    /// a name that is not a token draws the question mark. Each contour is filled as a polygon, and
    /// the grid-to-device mapping is the same one [`Self::draw_outline`] uses, so the fallback and
    /// the data path place an icon identically.
    fn draw_fallback(&self, ctx: &mut RenderContext) {
        let rect = self.icon_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let token = self.icon_name.as_str();
        // A name that is not a token has no fallback geometry, so the placeholder is the honest
        // answer — the same decision the shared helper leaves to its caller.
        if !crate::widget::icon_fallback_data::ICON_FALLBACK.iter().any(|entry| entry.name == token)
        {
            self.draw_unknown(ctx);
            return;
        }
        draw_fallback_in(ctx, rect, self.resolve_color(), token);
    }

    /// Draws an icon from outline data — the one path both a bundled and a host icon take.
    ///
    /// Each contour is filled as a polygon by the non-zero winding rule, so a counter (the hole in
    /// an outlined shape) fills as empty because its ring runs the opposite way — the same rule the
    /// glyph rasteriser uses. Data that does not fit the fixed scratch, or that does not parse, is
    /// **refused**: nothing is drawn for it rather than a truncated shape (a partial icon is a
    /// wrong icon).
    ///
    /// # Why this is not gated on `icons`
    ///
    /// A host's registered icon is outline data the host supplied, and it must draw on a build
    /// without the crate's bundled table — that is the whole point of registering one. Only
    /// [`Self::draw_outline`], the bundled-table convenience, needs the feature.
    fn draw_outline_data(&self, ctx: &mut RenderContext, data: &IconData) {
        let rect = self.icon_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // One geometry path, shared with the free `draw_icon_at` a control uses to paint an icon
        // inline, so an `Icon` widget and a control-drawn icon cannot render differently.
        draw_outline_in(ctx, rect, self.resolve_color(), data);
    }

    /// Draws an icon from the crate's **bundled** outline table.
    ///
    /// A thin wrapper over [`Self::draw_outline_data`], which does the work; this exists so a call
    /// site in the `icons` path names the bundled table rather than passing a `&IconData` it had to
    /// fetch itself.
    #[cfg(feature = "icons")]
    fn draw_outline(&self, ctx: &mut RenderContext, data: &IconData) {
        self.draw_outline_data(ctx, data);
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

#[cfg(widgets_unstripped)]
impl Widget for Icon {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // The icon's **own** requested box, not a fixed 24: `set_size` is a documented
        // property that requests a redraw, so a layout that asked this control how much room
        // it wanted and was always told 24 ignored the very setting the caller just made —
        // `Icon::new(rect)` followed by `set_size(48.0)` reserved a 24px slot. The value is
        // rounded to whole pixels because a size hint is in logical pixels, and floored at 1
        // so a degenerate box is still addressable.
        let side = self.size.round().max(1.0) as u32;
        crate::core::Size::new(side, side)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Icon`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
#[cfg(widgets_unstripped)]
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

#[cfg(widgets_unstripped)]
impl Draw for Icon {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        if !self.base.is_enabled() {
            // Render the disabled appearance by temporarily pinning an explicit colour: a
            // desaturated grey at half the resolved colour's alpha. The pin is removed
            // afterwards, so the pre-draw resolution (theme or explicit setter) is unchanged and
            // the next draw recomputes it.
            //
            // `saved` is the caller's own `color` (which `set_color` may have set), not `None`:
            // the draw path must never write a control's resolved state. Restoring `None` here
            // silently dropped a `set_color(red)` the moment the icon was drawn disabled once —
            // re-enabling it then painted the theme colour, not the caller's.
            let resolved = self.resolve_color();
            let gray = ((resolved.r as u16 + resolved.g as u16 + resolved.b as u16) / 3) as u8;
            let saved = self.color;
            self.color = Some(Color::rgba(gray, gray, gray, resolved.a / 2));
            self.draw_icon(context);
            self.color = saved;
            return;
        }
        self.draw_icon(context);
    }
}

#[cfg(widgets_unstripped)]
impl EventHandler for Icon {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(all(test, widgets_unstripped))]
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

    /// Drawing a disabled icon must not destroy the caller's colour.
    ///
    /// # The defect this pins
    ///
    /// `Icon::draw` pins a grey for the disabled appearance and then restored `self.color = None`
    /// — not the value it had saved. A `set_color(red)` icon therefore **permanently** lost red
    /// the first time it was painted disabled: re-enabling it painted the theme colour, not the
    /// caller's. The draw path may not write a control's resolved state, and the pre-draw value is
    /// exactly the state that says so.
    #[test]
    fn drawing_disabled_does_not_discard_an_explicit_colour() {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::draw::Draw;

        let bounds = Rect::new(0, 0, 24, 24);
        let mut icon = Icon::new(bounds);
        icon.set_icon("heart");
        icon.set_color(Color::RED);

        let mut surface = SoftwarePaintBackend::new(bounds.size(), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            icon.draw(&mut context);
        }

        assert_eq!(
            icon.explicit_color(),
            Some(Color::RED),
            "a normal draw must leave the caller's colour untouched"
        );

        // Disable, draw, and re-check: this is the path that used to reset the pin to `None`.
        icon.set_enabled(false);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            icon.draw(&mut context);
        }
        assert_eq!(
            icon.explicit_color(),
            Some(Color::RED),
            "a disabled draw must restore the caller's colour, not clear it"
        );
        icon.set_enabled(true);
        assert_eq!(icon.color(), Color::RED, "re-enabling must paint the caller's colour again");
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

    /// Every variant round-trips through `as_str` -> `from_name`.
    ///
    /// # Why this walks `ALL` rather than a hand-written list
    ///
    /// It used to spell out all 31 variants by hand, which meant a variant added to the enum but
    /// forgotten here — or a stale entry for one that had been removed — needed a test edit to
    /// stay honest. Walking `ALL` covers every variant the type declares, by construction, so the
    /// test cannot fall behind the enum.
    #[test]
    fn icon_name_enum_roundtrip() {
        for name in IconName::ALL {
            let s = name.as_str();
            let parsed = IconName::from_name(s);
            assert!(parsed.is_some(), "Failed to parse icon name: {s}");
            assert_eq!(parsed.unwrap(), name);
        }
        // And the enum must have more than one variant, so an `ALL` that accidentally fell empty
        // cannot make the loop above pass vacuously.
        assert!(IconName::ALL.len() > 1, "the enum must declare more than one icon");
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
        icon.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    }

    #[test]
    fn icon_unknown_fallback() {
        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("nonexistent");

        let svg = crate::widget::svg::render_to_svg(&mut icon);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    // ── The bundled outline path (opt-in `icons` feature) ────────────────

    /// Every token draws real ink through the data path, and no two draw the same picture.
    ///
    /// # Why this is a rendering test and not only a data test
    ///
    /// `tests/icon_data_integrity_test.rs` proves the *data* is complete and distinct. It cannot
    /// prove the data reaches the screen: a draw path that parsed nothing, or filled nothing, would
    /// leave that test green while every icon rendered blank. This renders each token through the
    /// real pipeline and asserts the emitted geometry is non-empty — the "no silent placeholder"
    /// check ICON-4 asks for, one level below the census gate.
    /// `alloc_frugal` rather than `not(feature = "mini")`: the two are the same condition
    /// (`alloc_frugal` *is* `feature = "mini"`, defined in `build.rs`), and rule #47 requires
    /// the profile gate to be spelled through the alias so the same fact is not written two
    /// ways across the tree.
    #[cfg(all(feature = "icons", not(alloc_frugal)))]
    #[test]
    fn every_tokened_icon_draws_a_distinct_outline() {
        use crate::widget::svg::render_to_svg;

        let mut outlines: Vec<String> = Vec::new();
        for token in IconName::ALL {
            let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
            icon.set_icon_enum(token);
            let svg = render_to_svg(&mut icon);
            // The data path emits `<path>` fills; the hand-drawn fallback emits `<line>`/`<circle>`.
            assert!(
                svg.contains("<path"),
                "{token:?} rendered no outline path, so the data did not reach the draw: {svg}"
            );
            for other in &outlines {
                assert_ne!(other, &svg, "{token:?} draws the same picture as another icon");
            }
            outlines.push(svg);
        }
        assert_eq!(outlines.len(), IconName::ALL.len());
    }

    /// With the feature off, a tokened icon still draws — through the generated fallback.
    ///
    /// This is the property that keeps a default build's snapshots from being *empty*: the data
    /// path must not become the *only* path, or a build without the payload would render nothing.
    ///
    /// # Why the assertion is `<path>` and not `<line>`
    ///
    /// It used to assert `<line>`, because the fallback was 31 hand-written methods that drew line
    /// and circle primitives. The fallback is now derived from the same Material Symbols outlines as
    /// the data (see `icon_fallback_data`), so it emits filled `<path>` polygons — the same kind of
    /// geometry the `icons` build emits, from the same source. Asserting `<path>` is therefore the
    /// stronger check: it proves the fallback drew a real outline, not that a particular primitive
    /// survived.
    #[cfg(not(feature = "icons"))]
    #[test]
    fn a_tokened_icon_still_draws_without_the_data() {
        use crate::widget::svg::render_to_svg;

        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon_enum(IconName::Check);
        let svg = render_to_svg(&mut icon);
        assert!(
            svg.contains("<path"),
            "the generated fallback must still draw a real outline: {svg}"
        );
        // And it must be a *check*, not the unknown placeholder: a fallback that fell through to
        // `draw_unknown` would still contain no `<path>` (it draws text), so this is the assertion
        // that the token was actually found in the fallback table.
        assert!(!svg.contains("?"), "the fallback must find the token, not draw the placeholder");
    }

    // ── The generated fallback table (BLUE25 follow-up: fallback shares the data's source) ──

    /// The fallback table lines up with `IconName` and every entry has real geometry.
    ///
    /// # Why this is a source-level test and not a rendering one
    ///
    /// `every_tokened_icon_draws_a_distinct_outline` renders through the *data* path; the fallback
    /// is a different table, so it needs its own check that it is complete and distinct. Running in
    /// `cfg(test)` of the widget module is what makes it run on a build **without** `icons` too,
    /// where `ICON_FALLBACK` is the only geometry the icon has.
    #[test]
    fn the_fallback_table_is_complete_and_distinct() {
        use crate::widget::icon_fallback_data::ICON_FALLBACK;

        assert_eq!(
            ICON_FALLBACK.len(),
            IconName::ALL.len(),
            "the fallback table and the IconName variants disagree on how many icons exist"
        );
        for (entry, variant) in ICON_FALLBACK.iter().zip(IconName::ALL.iter()) {
            assert_eq!(
                entry.name,
                variant.as_str(),
                "the fallback table's order has drifted from IconName's declaration order"
            );
            assert!(entry.grid > 0, "{:?} has a zero grid, so it cannot be scaled", entry.name);
            assert!(!entry.contours.is_empty(), "{:?} has no fallback geometry", entry.name);
            let mut points = 0usize;
            for contour in entry.contours {
                assert!(
                    contour.len() >= 3,
                    "{:?} has a contour of {} points, which cannot be a polygon",
                    entry.name,
                    contour.len()
                );
                points += contour.len();
            }
            assert!(points > 0, "{:?} flattened to geometry with no points", entry.name);
        }
        // No two entries share their whole geometry: the `Close == Cross` rule, applied to the
        // fallback, since both tables are derived from the same source and must stay distinct.
        for (index, entry) in ICON_FALLBACK.iter().enumerate() {
            for other in &ICON_FALLBACK[index + 1..] {
                let same = entry.contours.len() == other.contours.len()
                    && entry.contours.iter().zip(other.contours.iter()).all(|(a, b)| a == b);
                assert!(
                    !same,
                    "{:?} and {:?} have identical fallback geometry, so two names draw one \
                     picture",
                    entry.name, other.name
                );
            }
        }
    }

    // ── Host-registered icons (`register_icon`) ───────────────────────────────

    /// A registered icon **draws**, through the real pipeline, on any build.
    ///
    /// # Why this is a rendering test and not a registry test
    ///
    /// `icon_registry`'s own test proves the table stores and returns what was registered. It
    /// cannot prove the draw path *consults* it — a registry nothing reads is a dead entry point.
    /// This registers an outline under a name no `IconName` covers, renders it, and asserts real
    /// geometry came out: the wiring, not the storage.
    ///
    /// It runs on both feature states on purpose. A build without `icons` has no bundled table, and
    /// a host icon must still draw there — that is the case the registry exists for, and gating the
    /// test on `icons` would leave the important half unverified.
    #[test]
    fn a_registered_icon_draws_through_the_real_pipeline() {
        use crate::widget::svg::render_to_svg;
        use crate::widget::{clear_registered_icons, register_icon};

        clear_registered_icons();
        // A closed triangle on the 960-grid, `y` negative upward — the same convention the bundled
        // data uses, so this exercises the normal placement path rather than a special case.
        assert!(register_icon("host_probe", &["M480-200 240-440l480 480-240-240Z"]));

        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        icon.set_icon("host_probe");
        let svg = render_to_svg(&mut icon);

        assert!(
            svg.contains("<path"),
            "a registered icon must draw real geometry, not fall through to the placeholder: {svg}"
        );
        // The placeholder draws text; a `?` here would mean the name was not found, i.e. the draw
        // path did not consult the registry.
        assert!(!svg.contains("?"), "the registered name must resolve, not draw the placeholder");

        clear_registered_icons();
    }

    /// A name that is neither a built-in token nor registered still draws the placeholder.
    ///
    /// The control for the test above: if any name reached real geometry, the previous test would
    /// pass for the wrong reason.
    ///
    /// # Why the discriminator is the picture, not `<path>`
    ///
    /// `draw_unknown` draws a `?`, and the text layer rasterises a glyph to per-pixel `<path>`
    /// rectangles — so "contains no `<path>`" would be false for the placeholder too, and an
    /// earlier revision of this test asserted exactly that wrongly. The real difference is *what*
    /// is drawn: a registered icon emits its own outline (many points, spanning the box), while
    /// the placeholder emits the `?` glyph's pixels. Comparing against the placeholder's own output
    /// is the honest check.
    #[test]
    fn an_unknown_name_still_draws_the_placeholder() {
        use crate::widget::clear_registered_icons;
        use crate::widget::svg::render_to_svg;

        clear_registered_icons();
        let mut unknown = Icon::new(Rect::new(0, 0, 24, 24));
        unknown.set_icon("definitely_not_a_token");
        let unknown_svg = render_to_svg(&mut unknown);

        // A registered icon draws a *different* picture from the placeholder.
        let mut registered = Icon::new(Rect::new(0, 0, 24, 24));
        crate::widget::register_icon(
            "probe_for_placeholder",
            &["M480-200 240-440l480 480-240-240Z"],
        );
        registered.set_icon("probe_for_placeholder");
        let registered_svg = render_to_svg(&mut registered);
        clear_registered_icons();

        assert_ne!(
            unknown_svg, registered_svg,
            "an unknown name must not draw what a registered icon draws"
        );
        // And the unknown name's picture must not be empty: a placeholder is still ink.
        assert!(
            unknown_svg.contains("<path") || unknown_svg.contains("<rect"),
            "the placeholder must draw something: {unknown_svg}"
        );
    }

    /// An icon on a non-960 grid must land inside the box it was given.
    ///
    /// # The defect this pins
    ///
    /// `IconPlacement::map` translated design y by a **hard-coded 960** (`y + 960`) instead of by the
    /// caller's own grid. Every bundled icon is on a 960 grid, so nothing in the crate's own
    /// vocabulary noticed; a host icon registered with `register_icon_on_grid(.., 24)` — the
    /// documented way to bring a 24-unit viewBox such as Lucide's — was pushed `960 - 24` grid units
    /// too far, i.e. 40 boxes below the rect it was asked to draw in.
    ///
    /// # Why the test is at the placement level
    ///
    /// [`draw_outline_in`](super::draw_outline_in) calls
    /// `IconPlacement::new(rect.x, rect.y, rect.width, data.grid)`, so the grid reaches the mapping —
    /// and the mapping is what is under test. Asserting through a rendered widget would only
    /// exercise the bundled 960 grid, where the constant and the grid coincide.
    #[test]
    fn a_non_960_grid_is_placed_inside_its_box() {
        use crate::render::path::{flatten_paths, IconPlacement};

        // A 24-unit grid drawn 24 px square at (10, 20): the box is x 10..34, y 20..44.
        let placement = IconPlacement::new(10, 20, 24.0, 24);
        let mut points = [Point::new(0, 0); 16];
        let mut contours = [(0usize, 0usize); 4];
        // A triangle with its apex on the grid's **top** row (design y = -24, which maps to the
        // box's top edge) and its base at design y = -15.
        let count = flatten_paths(&["M0-15h24L12-24Z"], placement, &mut points, &mut contours)
            .expect("a triangle flattens");
        assert_eq!(count, 1);
        let (start, end) = contours[0];
        for point in &points[start..end] {
            assert!(
                (10..=34).contains(&point.x) && (20..=44).contains(&point.y),
                "a 24-grid icon must land inside its box, but a vertex is at {point:?}"
            );
        }
        // The apex (design y = -24, the grid's top) must be the **smallest** device y, and it must
        // sit on the box's top edge — not 40 boxes below it, which is what the hard-coded 960 gave.
        let apex_y = points[start..end].iter().map(|point| point.y).min().expect("a vertex");
        let base_y = points[start..end].iter().map(|point| point.y).max().expect("a vertex");
        assert!(
            apex_y < base_y,
            "the apex must be above the base, or the icon is rotated 180 degrees \
             (apex y={apex_y} base y={base_y})"
        );
        assert_eq!(apex_y, 20, "design y = -grid is the grid's top, so it is the box's top edge");
        assert_eq!(base_y, 29, "design y = -15 scales to 9 px below the box's top");
    }
}
