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
use crate::widget::icon_source::IconSource;
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
/// flattened through [`flatten_paths`](crate::render::path::flatten_paths) and filled by
/// [`RenderContext::fill_path_non_zero`](crate::render::RenderContext::fill_path_non_zero) — the
/// same non-zero winding rule the glyph rasteriser applies, which is the part the two share. (The
/// glyph rasteriser has its **own** outline flattener; the two produce the same packed
/// `(points, contours)` shape but are separate implementations, and this comment says so rather
/// than claiming a reuse that does not exist.) Without the feature (or for a name that
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
        if let Err(error) = draw_outline_in(ctx, rect, color, &data) {
            log::error!("icon '{}' could not be painted: {error:?}", data.name);
        }
        return;
    }
    draw_fallback_in(ctx, rect, color, name.as_str());
}

/// The resolved, borrowed view of an icon source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconRef<'a> {
    /// One of the crate's built-in names.
    Builtin(IconName),
    /// Validated or caller-supplied static path geometry.
    StaticGeometry(&'a IconData),
}

impl<'a> From<&'a IconData> for IconRef<'a> {
    fn from(data: &'a IconData) -> Self {
        Self::StaticGeometry(data)
    }
}

impl From<IconName> for IconRef<'_> {
    fn from(name: IconName) -> Self {
        Self::Builtin(name)
    }
}

impl From<crate::widget::icon_source::IconSource> for IconRef<'_> {
    fn from(source: crate::widget::icon_source::IconSource) -> Self {
        match source {
            crate::widget::icon_source::IconSource::Builtin(name) => Self::Builtin(name),
            crate::widget::icon_source::IconSource::StaticGeometry(data) => {
                Self::StaticGeometry(data)
            }
        }
    }
}

/// Result of attempting to paint an icon into a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconPaintOutcome {
    /// The source was drawn into a non-empty slot.
    Drawn,
    /// The slot had no area, so painting was skipped.
    SkippedEmptySlot,
}

/// Why a typed icon could not be painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconPaintError {
    /// Static path geometry is unavailable in this stripped build profile.
    UnsupportedSourceForProfile,
    /// The design grid is zero, so no device scale can be derived from it.
    ///
    /// A host-built [`IconData`] can carry `grid: 0`; the flattener treats that as a zero scale
    /// rather than panicking, which would silently flatten the whole outline onto one point. It is
    /// named here so a validation report says *why* instead of "empty".
    ZeroGrid,
    /// The path data was malformed or exceeded a geometry budget.
    InvalidGeometry(crate::render::path::FlattenError),
    /// The data parsed but contained no drawable contours.
    EmptyGeometry,
    /// The render command rejected the prepared compound path.
    Backend(crate::render::CompoundPathError),
}

/// The design facts about a source that can be checked **before** any slot size is known.
///
/// The first of §19.3's two validation stages; see [`validate_icon`] for exactly which questions it
/// answers and which it deliberately leaves to [`paint_icon`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconSourceMetadata {
    /// The source's design grid.
    pub grid: u16,
    /// How many closed contours the source declares.
    pub contour_count: usize,
}

/// The device box a source occupies inside a slot, and the design extent it came from.
///
/// What a caller asks *before* drawing, so the room it reserves and the glyph that lands in it come
/// from one rule instead of two. [`Icon::icon_rect`], `ToolButton::icon_rect` and `Chip::chip_width`
/// each derive their own box today; a caller that has to know where content will actually land asks
/// here rather than restating the fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconMetrics {
    /// The box the content occupies inside the slot it was measured for.
    pub bounds: Rect,
    /// The source's own extent in design units — its grid, which is square for geometry.
    pub source_extent: u32,
}

impl IconMetrics {
    /// Whether the slot had no area, i.e. nothing would be painted.
    ///
    /// A separate question from "did painting fail": [`paint_icon`] answers an empty slot with
    /// [`IconPaintOutcome::SkippedEmptySlot`], deliberately not an error, and a layout asks this for
    /// the same reason. Neither uses a failure to mean "no room".
    pub fn is_empty(&self) -> bool {
        self.bounds.width == 0 || self.bounds.height == 0
    }
}

/// The square a glyph is drawn in inside `slot` — the crate's one geometry fit rule.
///
/// `contain`, square, centred: the side is the slot's smaller dimension, so a slot that is not square
/// cannot stretch the glyph on one axis. The previous form scaled by `slot.width` on **both** axes,
/// so a 40x12 slot drew a 40x40 icon — 28 px taller than the box the caller reserved, clipped by
/// whatever the backend happened to clip.
///
/// **One control does pass a slot that is not square**, and this rule changes its pixels: `Stepper`
/// insets its `−`/`+` marks by `min(w, h) / 4` inside a 48x44 button, so each mark slot is 26x22.
/// Its glyphs therefore shrink from 26 px to 22 px and move to the centre of that slot — which is
/// the rule working, not a regression: a 26 px glyph does not fit a 22 px slot, and §19.4 requires a
/// paint to stay inside the slot the caller reserved. `snapshots/svg/stepper.svg` records the new
/// output, and `tools/check_svg_snapshots.sh` is what caught the change. The other slots the crate's
/// controls pass are square (`Icon::icon_rect`, `ToolButton::icon_rect`, `TAB_ICON_SIDE`,
/// `CHIP_ICON_SIDE`, …), so for them the rule is behaviour preserving.
fn glyph_box(slot: Rect) -> Rect {
    let side = slot.width.min(slot.height);
    Rect::new(
        slot.x + ((slot.width - side) / 2) as i32,
        slot.y + ((slot.height - side) / 2) as i32,
        side,
        side,
    )
}

/// The geometry a source resolves to, before any device size is applied.
enum SourceGeometry {
    /// SVG `d` on a design grid, drawn through the shared flattener.
    ///
    /// By value: [`IconData`] is `Copy` and its `paths` are `&'static`, so this carries the whole
    /// geometry without borrowing the table it came from.
    #[cfg(any(feature = "icons", widgets_unstripped))]
    Outline(IconData),
    /// The generated polygon table: already flattened, so its contour count is the table's.
    ///
    /// Borrowed, not copied: the table is `&'static`, and its grid travels with it so the extent a
    /// caller measures and the grid a draw scales by come from the same entry.
    Fallback(&'static crate::widget::icon_fallback_data::IconFallback),
}

/// Which geometry a source resolves to, or why it resolves to none.
fn source_geometry(source: IconRef<'_>) -> Result<SourceGeometry, IconPaintError> {
    match source {
        IconRef::Builtin(name) => {
            #[cfg(feature = "icons")]
            if let Some(data) = name.data_opt() {
                return Ok(SourceGeometry::Outline(data));
            }
            match crate::widget::icon_fallback_data::ICON_FALLBACK
                .iter()
                .find(|entry| entry.name == name.as_str())
            {
                // A token the generated table does not know has no geometry at all, which is a
                // failure rather than a zero-sized success: reporting it keeps "drew nothing" from
                // being indistinguishable from "drew an empty icon".
                None => Err(IconPaintError::EmptyGeometry),
                Some(entry) => Ok(SourceGeometry::Fallback(entry)),
            }
        }
        IconRef::StaticGeometry(data) => {
            #[cfg(any(feature = "icons", widgets_unstripped))]
            {
                Ok(SourceGeometry::Outline(*data))
            }
            #[cfg(not(any(feature = "icons", widgets_unstripped)))]
            {
                let _ = data;
                Err(IconPaintError::UnsupportedSourceForProfile)
            }
        }
    }
}

/// Measures where a source's content lands inside `slot`.
///
/// The measurement half of [`paint_icon`]: both call [`glyph_box`], so a reserved box and a painted
/// glyph cannot disagree. An empty slot measures empty rather than failing — see
/// [`IconMetrics::is_empty`].
///
/// A source with no drawable geometry is an error (`EmptyGeometry`, or
/// `UnsupportedSourceForProfile` on a build without an outline path), not a zero-sized box, so a
/// caller cannot mistake "this source is broken" for "there is no room".
pub fn measure_icon(source: IconRef<'_>, slot: Rect) -> Result<IconMetrics, IconPaintError> {
    let extent = match source_geometry(source)? {
        #[cfg(any(feature = "icons", widgets_unstripped))]
        SourceGeometry::Outline(data) => u32::from(data.grid),
        SourceGeometry::Fallback(entry) => u32::from(entry.grid),
    };
    Ok(IconMetrics { bounds: glyph_box(slot), source_extent: extent })
}

/// Validates a source's **size-independent** geometry, reporting what a draw would submit.
///
/// Stage one of §19.3's two, and deliberately only stage one. It answers the questions that are
/// properties of the *source*:
///
/// * the design grid is usable (non-zero);
/// * every `d` parses — which is also what rejects non-finite coordinates and features the parser
///   does not express;
/// * the geometry declares at least one contour, and no more than
///   [`MAX_OUTLINE_CONTOURS`](crate::render::path::MAX_OUTLINE_CONTOURS).
///
/// # What it deliberately does not answer, and why
///
/// The **point** budget is not a source property. The flattener subdivides curves to a fixed
/// device-space tolerance, so the vertex count depends on the box the icon is drawn in: a source
/// that fits the budget at 24 px can exceed it at 400 px, and a source whose whole outline is
/// smaller than one pixel at 24 px produces no contour at all while being perfectly valid data.
/// Judging either here would report a size as a property of the source. Both are therefore reported
/// by [`paint_icon`] for the size actually asked for, through
/// [`IconPaintError::InvalidGeometry`] and [`IconPaintError::EmptyGeometry`].
///
/// Contour count *is* a source property (one per `d` subpath) and is not rounded by device
/// placement, so it is checked here.
pub fn validate_icon(source: IconRef<'_>) -> Result<IconSourceMetadata, IconPaintError> {
    match source_geometry(source)? {
        #[cfg(any(feature = "icons", widgets_unstripped))]
        SourceGeometry::Outline(data) => {
            use crate::render::path::{parse, MAX_OUTLINE_CONTOURS};
            if data.grid == 0 {
                return Err(IconPaintError::ZeroGrid);
            }
            let mut contour_count = 0usize;
            for d in data.paths {
                let subpaths = parse(d).map_err(|error| {
                    IconPaintError::InvalidGeometry(crate::render::path::FlattenError::Parse(error))
                })?;
                for subpath in &subpaths {
                    if subpath.segments.is_empty() {
                        continue;
                    }
                    contour_count += 1;
                    if contour_count > MAX_OUTLINE_CONTOURS {
                        return Err(IconPaintError::InvalidGeometry(
                            crate::render::path::FlattenError::TooManyContours,
                        ));
                    }
                }
            }
            if contour_count == 0 {
                return Err(IconPaintError::EmptyGeometry);
            }
            Ok(IconSourceMetadata { grid: data.grid, contour_count })
        }
        SourceGeometry::Fallback(entry) => {
            if entry.grid == 0 {
                return Err(IconPaintError::ZeroGrid);
            }
            if entry.contours.is_empty() {
                return Err(IconPaintError::EmptyGeometry);
            }
            Ok(IconSourceMetadata { grid: entry.grid, contour_count: entry.contours.len() })
        }
    }
}

/// Paints a typed icon source into the supplied slot.
///
/// Geometry uses the same non-zero outline path as [`Icon`] and the legacy free drawing
/// functions. Built-in names retain the generated fallback when bundled outlines are disabled.
pub fn paint_icon(
    ctx: &mut RenderContext,
    source: IconRef<'_>,
    slot: Rect,
    color: Color,
) -> Result<IconPaintOutcome, IconPaintError> {
    if slot.width == 0 || slot.height == 0 {
        return Ok(IconPaintOutcome::SkippedEmptySlot);
    }

    match source {
        IconRef::Builtin(name) => {
            #[cfg(feature = "icons")]
            if let Some(data) = name.data_opt() {
                draw_outline_in(ctx, slot, color, &data)?;
                return Ok(IconPaintOutcome::Drawn);
            }
            // The generated table is what a build without the bundled outlines draws. Its `false`
            // means the token is in neither table -- there is no geometry at all -- which is an
            // `EmptyGeometry` failure, not a draw. Ignoring the return would make "nothing was
            // drawn" indistinguishable from "an icon was drawn", which is what §19.4 forbids an
            // outcome from doing.
            if draw_fallback_in(ctx, slot, color, name.as_str()) {
                Ok(IconPaintOutcome::Drawn)
            } else {
                Err(IconPaintError::EmptyGeometry)
            }
        }
        IconRef::StaticGeometry(data) => {
            #[cfg(any(feature = "icons", widgets_unstripped))]
            {
                draw_outline_in(ctx, slot, color, data)?;
                Ok(IconPaintOutcome::Drawn)
            }
            #[cfg(not(any(feature = "icons", widgets_unstripped)))]
            {
                let _ = (ctx, data, color);
                Err(IconPaintError::UnsupportedSourceForProfile)
            }
        }
    }
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
    match resolve_named_icon(name) {
        #[cfg(any(feature = "icons", widgets_unstripped))]
        Some(NamedIcon::Outline(data)) => {
            if let Err(error) = draw_outline_in(ctx, rect, color, &data) {
                log::error!("icon '{}' could not be painted: {error:?}", data.name);
            }
            true
        }
        Some(NamedIcon::Fallback) => {
            draw_fallback_in(ctx, rect, color, name);
            true
        }
        None => false,
    }
}

/// Whether a name would resolve to drawable icon geometry.
///
/// The measurement half of [`draw_icon_named`]: a layout that has to reserve room for an icon asks
/// this, and the paint asks the drawing function, and both read the same resolution so a reserved
/// slot and a painted glyph cannot disagree about whether a name is an icon.
pub fn icon_token_is_drawable(name: &str) -> bool {
    resolve_named_icon(name).is_some()
}

/// Draws a legacy widget icon [`Image`](crate::image::Image) into `slot`, reporting whether
/// anything was painted.
///
/// # The one seam for widgets that still hold an `Image`
///
/// A control that kept its old `Option<Image>` icon next to a typed [`IconSource`] used to repeat
/// this in its own `draw`. Several of them reserved the slot and then painted something else
/// entirely -- a placeholder dot -- or nothing at all, because the repeated attempt was never
/// finished. One function states the fit rule (`fit_image_into`) and the "the bytes are borrowed,
/// never transcoded per frame" rule once.
///
/// Returns `false` for a non-RGBA8 or empty image rather than painting a stand-in: that is the
/// honest answer for "this image cannot be drawn", and the caller decides what it means.
/// Normalising at assignment time (the crate's `normalize_rgba8_image`) is what keeps a realistic
/// setter from producing such an image in the first place.
#[cfg(feature = "image")]
pub fn draw_icon_image(
    context: &mut RenderContext,
    image: &crate::image::Image,
    slot: Rect,
) -> bool {
    let crate::image::ImageData::Rgba8(pixels) = &image.inner.data else {
        return false;
    };
    if pixels.is_empty() {
        return false;
    }
    let fitted = crate::widget::icon_source::fit_image_into(image.width(), image.height(), slot);
    context.draw_image(fitted.x, fitted.y, fitted.width, fitted.height, pixels);
    true
}

/// What a host-supplied icon name resolved to.
enum NamedIcon {
    /// A bundled or host-registered outline, drawn through `draw_outline_in`.
    ///
    /// Gated exactly like that function: a build with no outline path cannot resolve a name to one,
    /// so the variant is not merely unused there — it is unrepresentable.
    #[cfg(any(feature = "icons", widgets_unstripped))]
    Outline(IconData),
    /// Generated fallback geometry for a token with no outline.
    Fallback,
}

/// Resolves a host-supplied icon name, in the order both entry points document.
///
/// Factored out of [`draw_icon_named`] so the "is this name an icon" question and the "draw this
/// name as an icon" question cannot drift apart.
fn resolve_named_icon(name: &str) -> Option<NamedIcon> {
    // The bundled outline and the host registration both draw through `draw_outline_in`, which
    // exists only where an outline can be drawn at all. Gating this block the same way keeps a
    // build without an outline path compiling: there the token/registration lookups are skipped
    // and the generated fallback below answers instead, exactly as `Icon::draw_icon` does.
    #[cfg(any(feature = "icons", widgets_unstripped))]
    {
        #[cfg(feature = "icons")]
        if let Some(known) = IconName::from_name(name) {
            if let Some(data) = known.data_opt() {
                return Some(NamedIcon::Outline(data));
            }
        }
        // The runtime registry lives in the unstripped widget set — it is a lock and a table, which
        // a sized profile drops — while the bundled outlines come with `icons` alone. The two are
        // separate conditions, so a build with `icons` on a sized profile reaches the outlines and
        // no registry, which is what this gate says.
        #[cfg(widgets_unstripped)]
        if let Some(data) = crate::widget::display_widgets::icon_data_set::lookup_registered(name) {
            return Some(NamedIcon::Outline(data));
        }
    }
    // The generated fallback geometry is the answer on a build with no outline path, and the
    // last resort everywhere else. A name it does not know was never an icon, which is what `None`
    // reports to the caller.
    if crate::widget::icon_fallback_data::ICON_FALLBACK.iter().any(|entry| entry.name == name) {
        return Some(NamedIcon::Fallback);
    }
    None
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
fn draw_outline_in(
    ctx: &mut RenderContext,
    rect: Rect,
    color: Color,
    data: &IconData,
) -> Result<(), IconPaintError> {
    #[cfg(all(not(alloc_frugal), any(feature = "icons", widgets_unstripped)))]
    use crate::render::path::flatten::flatten_static_paths;
    #[cfg(alloc_frugal)]
    use crate::render::path::flatten_paths;
    use crate::render::path::{IconPlacement, MAX_OUTLINE_CONTOURS, MAX_OUTLINE_POINTS};

    // The shared fit rule: a square glyph centred in the slot, so the box `measure_icon` reports and
    // the box this draws in are the same box.
    let rect = glyph_box(rect);
    if rect.width == 0 || rect.height == 0 {
        return Err(IconPaintError::EmptyGeometry);
    }
    let placement = IconPlacement::new(rect.x, rect.y, rect.width as f32, data.grid);
    let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
    let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
    #[cfg(alloc_frugal)]
    let count = flatten_paths(data.paths, placement, &mut points, &mut contours);
    #[cfg(not(alloc_frugal))]
    let count = flatten_static_paths(data.paths, placement, &mut points, &mut contours);
    let count = count.map_err(IconPaintError::InvalidGeometry)?;
    if count == 0 {
        return Err(IconPaintError::EmptyGeometry);
    }
    let point_count = contours[..count].iter().map(|&(_, end)| end).max().unwrap_or(0);
    ctx.fill_path_non_zero(&points[..point_count], &contours[..count], color)
        .map_err(IconPaintError::Backend)
}

/// Draws `token`'s generated fallback geometry into `rect`; unknown tokens draw nothing here
/// (the caller falls back to the placeholder).
fn draw_fallback_in(ctx: &mut RenderContext, rect: Rect, color: Color, token: &str) -> bool {
    use crate::widget::icon_fallback_data::ICON_FALLBACK;

    let Some(fallback) = ICON_FALLBACK.iter().find(|entry| entry.name == token) else {
        return false;
    };
    // The same square fit the outline path uses, so the two sources cannot land in different boxes.
    let rect = glyph_box(rect);
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let mut points = crate::compat::Vec::new();
    let mut contours = crate::compat::Vec::new();
    let grid = f32::from(fallback.grid);
    let scale = rect.width as f32 / grid;
    let origin_x = rect.x as f32;
    let origin_y = rect.y as f32;
    for contour in fallback.contours {
        if contour.len() < 3 {
            continue;
        }
        let start = points.len();
        points.extend(contour.iter().map(|&(gx, gy)| {
            Point::new(
                (origin_x + gx as f32 * scale).round() as i32,
                (origin_y + (gy as f32 + grid) * scale).round() as i32,
            )
        }));
        contours.push((start, points.len()));
    }
    if let Err(error) = ctx.fill_path_non_zero(&points, &contours, color) {
        log::error!("fallback icon '{}' could not be submitted: {error:?}", fallback.name);
    }
    true
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
    icon_source: Option<IconSource>,
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
            icon_source: None,
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
        if self.icon_name == name && self.icon_source.is_none() {
            return;
        }
        self.icon_name = name.to_string();
        self.icon_source = None;
        self.base.request_redraw();
    }

    /// Returns the current legacy name, or the canonical token for a typed built-in source.
    ///
    /// For the legacy string setter, this is whatever was last passed (or the default `"check"`),
    /// not a canonicalised token: an unrecognised name is returned as-is even though it draws as
    /// the placeholder. Typed built-in sources return their canonical token.
    pub fn icon(&self) -> &str {
        match self.icon_source {
            Some(IconSource::Builtin(name)) => name.as_str(),
            Some(IconSource::StaticGeometry(_)) => "",
            None => &self.icon_name,
        }
    }

    /// Sets the icon using an `IconName` enum variant.
    ///
    /// Equivalent to [`Icon::set_icon`] with the variant's canonical string, so
    /// the name always resolves to a real icon.
    pub fn set_icon_enum(&mut self, icon: IconName) {
        self.set_icon(icon.as_str());
    }

    /// Sets the icon from a built-in token or static geometry.
    ///
    /// This replaces any previous legacy name. `icon()` continues to return the canonical token
    /// for `Builtin`; it returns an empty string for custom geometry, which has no token name.
    pub fn set_icon_source(&mut self, source: IconSource) {
        self.icon_name.clear();
        self.icon_source = Some(source);
        self.base.request_redraw();
    }

    /// Returns the typed icon source, if one was most recently assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Clears both the typed source and legacy icon name.
    pub fn clear_icon_source(&mut self) {
        self.icon_source = None;
        self.icon_name.clear();
        self.base.request_redraw();
    }

    /// Sets the icon size in logical pixels.
    ///
    /// The size is the length of the icon's square bounding box, not its
    /// stroke or glyph height. Values are clamped to a minimum of `4.0` to keep
    /// the icon legible. Requests a redraw.
    pub fn set_size(&mut self, size: f32) {
        let size = size.max(4.0);
        if self.size == size {
            return;
        }
        self.size = size;
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
        if self.color == Some(color) {
            return;
        }
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
        if let Some(source) = self.icon_source {
            if let Err(error) =
                paint_icon(ctx, source.into(), self.icon_rect(), self.resolve_color())
            {
                log::error!("typed icon could not be painted: {error:?}");
            }
            return;
        }
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
    /// all contours are filled together, so oppositely-wound counters remain empty. A name that is
    /// not a token draws the question mark.
    fn draw_fallback(&self, ctx: &mut RenderContext) {
        let rect = self.icon_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let token = self.icon_name.as_str();
        if !draw_fallback_in(ctx, rect, self.resolve_color(), token) {
            self.draw_unknown(ctx);
        }
    }

    /// Draws an icon from outline data — the one path both a bundled and a host icon take.
    ///
    /// All contours are submitted as one non-zero winding fill, so opposite-wound counters remain
    /// empty. Data that does not fit the fixed scratch, or that does not parse, is refused rather
    /// than truncated; the failure is reported through the crate logger.
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
        if let Err(error) = draw_outline_in(ctx, rect, self.resolve_color(), data) {
            log::error!("icon '{}' could not be painted: {error:?}", data.name);
        }
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
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                // A borrowed static-geometry source has no document representation, so reading it
                // back as one would be a lie; the caller is told instead.
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
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
            "icon_source" => {
                match value {
                    // `Icon` keeps one active icon state, so clearing the typed source clears the
                    // legacy name with it — the setter pair documents that rule.
                    CapabilityValue::Null => self.clear_icon_source(),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(IconSource::Builtin(name)),
                    // A resource ID needs a host resolver; turning it into a path here would be
                    // guessing at the host's resource policy.
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "size" => {
                self.set_size(expect_f64(value)? as f32);
                Ok(())
            }
            "color" => match value {
                // `Null` restores "follow the theme", which is the state a fresh icon
                // starts in; the getter then reports whatever the theme resolves.
                CapabilityValue::Null => {
                    self.clear_color();
                    Ok(())
                }
                other => {
                    let raw = expect_string(other)?;
                    let color =
                        Color::parse_hex(&raw).ok_or(CapabilityAccessError::TypeMismatch)?;
                    self.set_color(color);
                    Ok(())
                }
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["icon_name", "icon_source", "size", "color", BASE_PROPERTY_NAMES]
    }

    /// Applies resolver-produced content through the same `icon_source` property the host reads.
    fn set_resolved_icon_source(
        &mut self,
        source: Option<IconSource>,
    ) -> Result<(), CapabilityAccessError> {
        match source {
            Some(source) => self.set_icon_source(source),
            None => self.clear_icon_source(),
        }
        Ok(())
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
    fn static_compound_icon_preserves_holes_and_replaces_legacy_state() {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::draw::Draw;

        static RING: IconData =
            IconData { name: "ring", grid: 24, paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"] };
        let bounds = Rect::new(0, 0, 24, 24);
        let mut icon = Icon::new(bounds);
        let source = IconSource::static_geometry(&RING);
        icon.set_icon_source(source);
        assert_eq!(icon.icon_source(), Some(source));
        assert!(icon.icon().is_empty(), "static geometry has no legacy token name");

        let mut backend = SoftwarePaintBackend::new(bounds.size(), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        {
            let mut context = RenderContext::new(&mut backend);
            icon.draw(&mut context);
        }
        backend.end_frame();
        let pixels = backend.frame_rgba();
        let alpha_at = |x: usize, y: usize| pixels[(y * 24 + x) * 4 + 3];
        assert_eq!(alpha_at(12, 12), 0, "opposite-wound inner contours must remain transparent");
        assert_eq!(alpha_at(3, 12), 255, "the surrounding contour must be filled");

        icon.set_icon("check");
        assert_eq!(icon.icon_source(), None, "a legacy setter replaces the typed source");
        assert_eq!(icon.icon(), "check");
        icon.set_icon_source(source);
        icon.clear_icon_source();
        assert_eq!(icon.icon_source(), None);
        assert!(icon.icon().is_empty(), "clearing removes rather than revives a prior source");

        let builtin = IconSource::builtin(IconName::Info);
        icon.set_icon_source(builtin);
        assert_eq!(icon.icon_source(), Some(builtin));
        assert_eq!(icon.icon(), "info", "typed built-ins expose their canonical legacy token");
        icon.set_icon("warning");
        assert_eq!(icon.icon_source(), None, "the legacy setter also replaces a typed built-in");
        assert_eq!(icon.icon(), "warning");
    }

    #[test]
    fn the_icon_source_property_round_trips_and_refuses_a_runtime_only_source() {
        use crate::widget::icon_source::{IconColorMode, IconSpec};

        static RING: IconData =
            IconData { name: "ring", grid: 24, paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"] };

        let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
        assert_eq!(icon.get("icon_source"), Ok(CapabilityValue::Null), "a fresh icon has none");
        assert!(icon.property_names().contains(&"icon_source"), "the name must be published");

        // The persisted value is what a document carries, so it must survive the write *and* the
        // read, and the control's own getter must agree with it.
        icon.set("icon_source", CapabilityValue::IconSpec(IconSpec::Builtin(IconName::Info)))
            .expect("a built-in token is persistable");
        assert_eq!(icon.icon_source(), Some(IconSource::Builtin(IconName::Info)));
        assert_eq!(
            icon.get("icon_source"),
            Ok(CapabilityValue::IconSpec(IconSpec::Builtin(IconName::Info)))
        );
        assert_eq!(icon.icon(), "info", "one active state: the legacy name reports the token");

        // A resource descriptor needs the host's resolver, which a property write does not have.
        assert_eq!(
            icon.set(
                "icon_source",
                CapabilityValue::IconSpec(IconSpec::Resource {
                    id: "project/icons/a".into(),
                    color_mode: IconColorMode::Monochrome,
                })
            ),
            Err(CapabilityAccessError::ResourceResolutionRequired)
        );

        // A runtime-only source has no document form, so reading it back refuses rather than
        // reporting `Null`, which a caller cannot tell apart from "this icon has no source".
        icon.set_icon_source(IconSource::static_geometry(&RING));
        assert_eq!(icon.get("icon_source"), Err(CapabilityAccessError::IconSourceNotPersistable));

        // `Null` clears both channels, so they cannot disagree after a reset.
        icon.set("icon_source", CapabilityValue::Null).expect("null clears the source");
        assert_eq!(icon.icon_source(), None);
        assert!(icon.icon().is_empty(), "clearing the typed source clears the legacy name too");
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

    // ── Measure / validate: one box rule, two honest stages ───────────────────

    /// A slot that is not square must not stretch the glyph, and the box `measure_icon` reports
    /// must be the box a draw lands in — otherwise a caller reserving room from the measurement
    /// would reserve the wrong room.
    #[cfg(feature = "icons")]
    #[test]
    fn measure_icon_is_square_contain_and_matches_where_the_glyph_is_drawn() {
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        let wide = Rect::new(10, 20, 40, 12);
        let metrics =
            measure_icon(IconRef::Builtin(IconName::Info), wide).expect("a bundled token measures");
        assert_eq!(
            (metrics.bounds.width, metrics.bounds.height),
            (12, 12),
            "the square side is the slot's smaller dimension"
        );
        assert_eq!((metrics.bounds.x, metrics.bounds.y), (24, 20), "centred in the slot");
        assert!(!metrics.is_empty());

        // Where the glyph actually lands: the rasterised ink must stay inside the measured box.
        // Reading the pixels rather than the geometry is what makes this a check of the draw and not
        // a restatement of the measurement.
        let mut backend = SoftwarePaintBackend::new(Size::new(64, 40), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        {
            let mut ctx = RenderContext::new(&mut backend);
            paint_icon(&mut ctx, IconRef::Builtin(IconName::Info), wide, Color::BLACK)
                .expect("the bundled outline paints");
        }
        backend.end_frame();
        let mut ink = 0usize;
        let (left, top) = (metrics.bounds.x, metrics.bounds.y);
        let (right, bottom) =
            (left + metrics.bounds.width as i32, top + metrics.bounds.height as i32);
        for y in 0..40i32 {
            for x in 0..64i32 {
                let alpha = PaintBackend::frame_rgba(&backend)[((y * 64 + x) * 4 + 3) as usize];
                if alpha == 0 {
                    continue;
                }
                ink += 1;
                assert!(
                    (left..right).contains(&x) && (top..bottom).contains(&y),
                    "ink at ({x}, {y}) escapes the measured box {left}..{right} x {top}..{bottom}"
                );
            }
        }
        assert!(ink > 0, "the glyph must actually paint inside its box");

        // A square slot measures back to itself, which is what every existing caller passes and why
        // this rule is behaviour preserving for them.
        let square = Rect::new(4, 5, 16, 16);
        assert_eq!(
            measure_icon(IconRef::Builtin(IconName::Save), square).expect("measures").bounds,
            square
        );
        assert_eq!(metrics.source_extent, u32::from(IconName::Info.data().grid));
    }

    #[test]
    fn measure_icon_reports_an_empty_slot_as_empty_rather_than_failing() {
        let metrics = measure_icon(IconRef::Builtin(IconName::Info), Rect::new(0, 0, 0, 8))
            .expect("an empty slot is not an error");
        assert!(metrics.is_empty());
    }

    /// Stage one answers the source questions, including the contour budget, which is not rounded by
    /// device placement.
    #[cfg(feature = "icons")]
    #[test]
    fn validate_icon_reports_the_source_budget_and_refuses_the_undrawable() {
        let metadata =
            validate_icon(IconRef::Builtin(IconName::Info)).expect("a bundled token validates");
        assert_eq!(metadata.grid, IconName::Info.data().grid);
        assert!(metadata.contour_count >= 1);

        static HOLE: IconData = IconData {
            name: "validate-hole",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };
        let two = validate_icon(IconRef::StaticGeometry(&HOLE)).expect("two subpaths validate");
        assert_eq!(two.contour_count, 2, "one contour per `d` subpath");
        assert_eq!(two.grid, 24);

        // Zero grid, unparseable data, and no geometry at all are three distinct refusals.
        static ZERO_GRID: IconData = IconData { name: "zero", grid: 0, paths: &["M0 0L1 0Z"] };
        assert_eq!(
            validate_icon(IconRef::StaticGeometry(&ZERO_GRID)),
            Err(IconPaintError::ZeroGrid)
        );

        static BROKEN: IconData = IconData { name: "broken", grid: 24, paths: &["M0 0L oops"] };
        assert!(matches!(
            validate_icon(IconRef::StaticGeometry(&BROKEN)),
            Err(IconPaintError::InvalidGeometry(crate::render::path::FlattenError::Parse(_)))
        ));

        // An empty `d` parses and declares no contour, so it is `EmptyGeometry` rather than a
        // passing zero-contour success.
        static NO_CONTOURS: IconData = IconData { name: "none", grid: 24, paths: &[""] };
        assert_eq!(
            validate_icon(IconRef::StaticGeometry(&NO_CONTOURS)),
            Err(IconPaintError::EmptyGeometry)
        );
    }

    /// A source that is valid but finer than one device pixel at a small size really does draw
    /// nothing — and that is **stage two**'s answer, reported at paint time for the size asked for,
    /// not a stage-one refusal of the source. Both halves are asserted so neither can quietly take
    /// over the other's job.
    #[test]
    fn a_source_too_fine_for_the_slot_is_refused_at_paint_time_not_at_validation() {
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        static TINY: IconData = IconData { name: "tiny", grid: 960, paths: &["M0 0L10 0L10 -10Z"] };
        assert!(
            validate_icon(IconRef::StaticGeometry(&TINY)).is_ok(),
            "the source itself is valid"
        );

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 24), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        let mut ctx = RenderContext::new(&mut backend);
        assert_eq!(
            paint_icon(
                &mut ctx,
                IconRef::StaticGeometry(&TINY),
                Rect::new(0, 0, 24, 24),
                Color::BLACK
            ),
            Err(IconPaintError::EmptyGeometry),
            "at 24 px the whole outline is sub-pixel, which paint reports for that size"
        );
    }

    /// An icon the geometry table does not know must not report a successful draw.
    #[test]
    fn painting_a_token_with_no_geometry_is_a_failure_not_a_silent_success() {
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        // Every `IconName` variant is covered by the generated table (asserted elsewhere), so this
        // exercises the *token* path through a name that is not one: `draw_icon_named` returns
        // `false` for it, and `paint_icon`'s fallback arm must not claim `Drawn`.
        let mut backend = SoftwarePaintBackend::new(Size::new(24, 24), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        let mut ctx = RenderContext::new(&mut backend);
        assert!(!draw_icon_named(&mut ctx, Rect::new(0, 0, 24, 24), Color::BLACK, "not-an-icon"));
        assert!(!icon_token_is_drawable("not-an-icon"));
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
