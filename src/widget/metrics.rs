// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Content-driven control metrics — the "implicit size" system.
//!
//! # Why this module exists
//!
//! A control is given a rectangle by whatever placed it (a layout, a designer, a
//! census cell), and that rectangle is its **available area** — how much room it
//! may occupy, not how big it should draw. Those are two different questions, and
//! conflating them is what turned `switch.svg` into a 240x120 stadium: the census
//! hands every control a 240x120 box, and a control that treats that box as a
//! *drawing instruction* paints a 240-wide track.
//!
//! Qt Quick's `Button.qml` answers the second question with
//!
//! ```text
//! implicitWidth = max(implicitBackgroundWidth + leftInset + rightInset,
//!                     implicitContentWidth   + leftPadding + rightPadding)
//! ```
//!
//! and the key term is the `max`: the **background is a minimum tappable
//! floor**, not decoration. That is why a button labelled with 5 px text is
//! still 100x40 rather than 100x18.
//!
//! [`ControlMetrics`] is the Rust-native spelling of that formula: no trait
//! objects, no virtual dispatch, just pure functions over value types.
//!
//! The [`metrics`] constants alongside it are the numeric half of the same
//! system — the agreed size of a switch track, a checkbox box, a progress bar.
//! They are constants rather than per-control literals so that the answer to
//! "how thick is a progress bar" exists in exactly one place (rule #101).
//!
//! # Removed: `ControlMetrics::drawn_box`
//!
//! There used to be a `drawn_box(rect, intrinsic)` that returned
//! `center_in(rect, intrinsic)` verbatim — an alias with **zero call sites** in the crate.
//! Rule #51 asks a shared abstraction to earn its place by eliminating a real
//! duplication; a second name for one function eliminates nothing and only leaves
//! two spellings of the same derivation to drift apart (rule #101). It is therefore
//! deleted rather than kept "for symmetry". A control that wants a fixed piece of
//! chrome centred reaches for [`ControlMetrics::center_in`]; a **panel** reaches for
//! [`ControlMetrics::painted_box`], which differs in the one way a panel needs — it is
//! clamped up to a one-pixel floor so a squeezed dialog stays visible instead of
//! collapsing to a zero-extent, invisible rect.

use crate::core::{Rect, Size};
use crate::style::EdgeOffsets;

/// Content-driven sizing for a single control.
///
/// Both functions are pure and allocation-free, so calling them from a hot
/// layout path costs nothing beyond the arithmetic.
pub struct ControlMetrics;

impl ControlMetrics {
    /// Intrinsic size = `max(floor, content + padding)`, component-wise.
    ///
    /// This is the whole point of Qt's `Button.qml` formula: the **floor is a
    /// minimum tappable area**, so small content does not shrink the control below
    /// what a finger can address, while large content still grows it past the
    /// floor. A control whose content is tiny and whose floor is `64x40` is
    /// `64x40`; a control whose content plus padding exceeds the floor is exactly
    /// that much.
    ///
    /// `padding` is applied to both sides of an axis, so a uniform horizontal
    /// padding of 12 adds 24 to the width.
    pub fn implicit_size(content: Size, padding: EdgeOffsets, floor: Size) -> Size {
        let padded_width = content.width.saturating_add(padding.horizontal_total());
        let padded_height = content.height.saturating_add(padding.vertical_total());
        Size::new(padded_width.max(floor.width), padded_height.max(floor.height))
    }

    /// The box left for content once `padding` is removed from `rect`.
    ///
    /// Qt calls this `availableWidth` / `availableHeight` (`qquickcontrol.cpp:382`).
    /// The result is never negative: padding larger than the rectangle collapses
    /// the content box to zero rather than inverting it, because a negative extent
    /// is a drawing instruction that would paint outside the control.
    pub fn content_box(rect: Rect, padding: EdgeOffsets) -> Rect {
        Rect::new(
            rect.x.saturating_add(padding.left as i32),
            rect.y.saturating_add(padding.top as i32),
            rect.width.saturating_sub(padding.horizontal_total()),
            rect.height.saturating_sub(padding.vertical_total()),
        )
    }

    /// Centres a `floor`-sized box inside `rect`, clamped to fit.
    ///
    /// A control that owns a fixed-size piece of chrome (a 52x32 switch track, an
    /// 18x18 checkbox indicator) draws that chrome centred in its available area
    /// rather than stretched across it. When the available area is smaller than the
    /// chrome the result is clamped, never expanded: nothing clips a widget at this
    /// layer, so painting outside the rectangle would be a layout violation rather
    /// than a graceful degradation.
    pub fn center_in(rect: Rect, floor: Size) -> Rect {
        let width = floor.width.min(rect.width);
        let height = floor.height.min(rect.height);
        Rect::new(
            rect.x + (rect.width.saturating_sub(width) / 2) as i32,
            rect.y + (rect.height.saturating_sub(height) / 2) as i32,
            width,
            height,
        )
    }

    /// The box a fixed-size leading affordance occupies, vertically centred on `line_y`'s row.
    ///
    /// # Why the box is not clamped to the row's height
    ///
    /// A checkbox indicator is 18x18 whatever the line is: its height is part of the control's
    /// identity, not a fraction of the text beside it. Clamping to the line box produced an
    /// **18x14** indicator on a 14 px font — a rectangle pretending to be a square — which is
    /// visible in the snapshot as a squashed box. The row's height decides *where* the box sits;
    /// the box's own size decides how big it is. Only the control's own rectangle can clamp it.
    pub fn leading_box(rect: Rect, size: Size, padding: EdgeOffsets) -> Rect {
        let width = size.width.min(rect.width);
        let height = size.height;
        // Centre on `rect`'s own row, which the caller has already sized to the line.
        let y = rect.y + (rect.height.saturating_sub(height) / 2) as i32;
        let x = rect.x.saturating_add(padding.left as i32);
        // Keep the box inside the rectangle when the left padding alone would push it out; a
        // leading affordance that is not visible is not an affordance.
        let x = x.min(rect.x + rect.width.saturating_sub(width) as i32);
        Rect::new(x, y, width, height)
    }

    /// The square box a fixed-diameter disc occupies, centred horizontally in `rect`.
    ///
    /// # Why the diameter is not clamped to the row
    ///
    /// A radio's ring is a fixed size for the same reason a checkbox's box is: it is chrome the
    /// control owns. Clamping it to the label's line height made the ring smaller than the
    /// checkbox box it sits beside in the same form.
    pub fn centered_disc(rect: Rect, diameter: u32) -> Rect {
        // Only the rectangle's own extent can clamp a disc: a diameter larger than the control
        // would paint outside it, and nothing clips a widget at this layer.
        let diameter = diameter.min(rect.width).min(rect.height);
        Rect::new(
            rect.x + (rect.width.saturating_sub(diameter) / 2) as i32,
            rect.y + (rect.height.saturating_sub(diameter) / 2) as i32,
            diameter,
            diameter,
        )
    }

    /// A band of `height` centred vertically in `rect`.
    ///
    /// Progress bars, sliders, dividers and scrollbar tracks are all "a line of
    /// this thickness across the middle of my area". Centring here means the band
    /// does not depend on the container's height, which is what stops a 240x120
    /// census cell from turning a 4 px bar into a 120 px slab.
    pub fn centered_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(
            rect.x,
            rect.y + (rect.height.saturating_sub(height) / 2) as i32,
            rect.width,
            height,
        )
    }

    /// The drawn box for a control whose chrome is a single horizontal band.
    ///
    /// A field, a toolbar or a row is *supposed* to span its width and take a fixed
    /// height; only the height is a control fact. `size_hint` reports that height when
    /// the width is not content-driven, so this keeps the full width and centres the
    /// band vertically — which is exactly what stops a 48 px field from drawing as a
    /// 120 px slab.
    pub fn full_width_band(rect: Rect, height: u32) -> Rect {
        Self::centered_band(rect, height)
    }

    /// A square box of `size` centred in `rect`.
    pub fn centered_square(rect: Rect, size: u32) -> Rect {
        Self::center_in(rect, Size::new(size, size))
    }

    /// A full-width band of `height` pinned to the **top** of `rect`.
    ///
    /// # Why this is not [`Self::full_width_band`]
    ///
    /// A tab strip, a navigation bar, a menu-bar entry row and a page header are not
    /// centered chrome: they are pinned to one edge by contract, because whatever the
    /// layout handed them, the content they label begins at their trailing edge. A nav
    /// bar centred in a 120 px census cell would put its titles halfway down the control
    /// with a strip of nothing above them, and `content_rect` — which starts at
    /// `band.bottom()` — would place the page *over* the bar.
    ///
    /// The band never leaves `rect`: when `height` exceeds the rectangle the result is
    /// clamped to it, because nothing clips a widget at this layer and painting outside
    /// the given area is a layout violation rather than a graceful degradation.
    pub fn top_band(rect: Rect, height: u32) -> Rect {
        Rect::new(rect.x, rect.y, rect.width, height.min(rect.height))
    }

    /// A full-width band of `height` pinned to the **bottom** of `rect`.
    ///
    /// The mirror of [`Self::top_band`], for the strip a layout pins to the bottom edge
    /// (a status bar, a bottom tab strip). `content_rect` then ends at `band.y`.
    pub fn bottom_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(rect.x, rect.y + rect.height.saturating_sub(height) as i32, rect.width, height)
    }

    /// The region inside `band` once `inset` is removed from each of the band's own edges.
    ///
    /// # Why an inset is measured from the band and not from the control's rectangle
    ///
    /// A tab's fill, a toolbar item's hover square and a page header's label are all "my
    /// band, minus its own edging". Deriving that from the *control's* rectangle is what
    /// made a 24 px `TabWidget` tab sit at y 96 in a 120 px cell: the strip was placed
    /// correctly and then re-anchored to the control's bottom edge, moving every tab away
    /// from the strip it belongs to. Taking the band as the input makes that impossible to
    /// express.
    ///
    /// `inset` is applied to all four edges, so an inset of 2 removes 4 from each axis.
    /// The result is never inverted.
    pub fn band_inset(band: Rect, inset: u32) -> Rect {
        Rect::new(
            band.x.saturating_add(inset as i32),
            band.y.saturating_add(inset as i32),
            band.width.saturating_sub(2 * inset),
            band.height.saturating_sub(2 * inset),
        )
    }

    /// The rectangle a bottom-pinned strip of `height` leaves for content above it.
    ///
    /// The companion to [`Self::bottom_band`]: both are derived from the same `height`, so
    /// the strip and the content it pushed up cannot overlap or leave a gap between them.
    pub fn content_above_bottom_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(height))
    }

    /// The rectangle a top-pinned strip of `height` leaves for content below it.
    ///
    /// The companion to [`Self::top_band`]. Returning `rect` unchanged when the strip is
    /// taller than the rectangle is deliberate: the alternative, a negative extent, is a
    /// drawing instruction that would paint outside the control.
    pub fn content_below_top_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(
            rect.x,
            rect.y.saturating_add(height as i32),
            rect.width,
            rect.height.saturating_sub(height),
        )
    }

    /// The box a **dialog or panel** actually paints: at most `intrinsic` in each axis,
    /// centred in `rect`, and never zero in either axis.
    ///
    /// # Why a panel does not fill its rectangle
    ///
    /// A dialog is handed a rectangle by whatever placed it (a census cell, a layout slot),
    /// and that rectangle is its **available area**, not a drawing instruction. QDialog,
    /// Flutter's `Dialog` and SwiftUI's `.alert` all have an intrinsic size and are centred
    /// in the room they are offered; a dialog that stretches to a 240x120 census cell draws
    /// a frame shaped like a dialog rather than a dialog. This is the same rule
    /// [`ControlMetrics::center_in`] applies to a fixed piece of chrome, with one addition that matters for
    /// a panel: the result is clamped *up* to one pixel, because a zero-extent rect is an
    /// **invisible** element, and a dialog that paints nothing is a defect rather than a
    /// tight fit.
    ///
    /// # Why the height is not forced to the floor
    ///
    /// Unlike a button's tappable floor, a dialog has no minimum size of its own: a panel
    /// that ignores the caller's height would paint outside the area it was given, since
    /// nothing clips a widget at this layer. `intrinsic` is therefore a *cap*, not a floor
    /// in the vertical axis — `DIALOG_MIN_WIDTH` is the one dimension a dialog does claim,
    /// and it is clamped to `rect` like every other size here.
    ///
    /// The one-pixel floor is applied **after** the centring, not before it: `center_in`
    /// clamps its result to `rect`, so a caller that handed this a zero-extent rectangle
    /// would have had the floor clamped back to zero. A panel that is one pixel on screen
    /// is visible; one that is zero pixels is the defect this exists to prevent.
    pub fn painted_box(rect: Rect, intrinsic: Size) -> Rect {
        let boxed = Self::center_in(rect, intrinsic);
        Rect::new(boxed.x, boxed.y, boxed.width.max(1), boxed.height.max(1))
    }

    /// The rectangle a focus ring occupies for a control of `rect`.
    ///
    /// # Why the ring is drawn *inside* the control's rectangle
    ///
    /// Qt Quick offsets its focus frame so it surrounds the control's background
    /// (`qquickcontrol.cpp` `focusFrame`, which grows the frame by the padding).
    /// This crate does not clip a child to its layout slot, so a ring drawn outside
    /// the rectangle would overlap whatever the layout placed next to the control —
    /// and on a toolbar, where controls sit `spacing` px apart, that overlap would be
    /// visible. Insetting by [`FOCUS_RING_WIDTH`] keeps the ring within the area the
    /// control was given.
    pub fn focus_ring_rect(rect: Rect) -> Rect {
        let inset = FOCUS_RING_WIDTH as i32;
        Rect::new(
            rect.x.saturating_add(inset),
            rect.y.saturating_add(inset),
            rect.width.saturating_sub(2 * FOCUS_RING_WIDTH),
            rect.height.saturating_sub(2 * FOCUS_RING_WIDTH),
        )
    }

    /// The corner radius a focus ring should follow for a control drawn with `radius`.
    ///
    /// The ring sits inside the control's edge, so its corners must be correspondingly
    /// tighter; reusing the control's own radius would make the ring bulge past the
    /// corners it is supposed to follow. The width is subtracted once rather than
    /// scaled, so a square control (radius 0) keeps square corners and a stadium
    /// stays a stadium.
    pub fn focus_ring_radius(radius: u32) -> u32 {
        radius.saturating_sub(FOCUS_RING_WIDTH)
    }
}

/// Thickness of a keyboard focus ring: 2 logical px.
///
/// Two is the smallest width that reads as deliberate rather than as a rendering
/// artefact on a 1x display, and it is what Qt's Basic style uses for its focus
/// frame. Named here rather than at the draw site so every control that draws a ring
/// agrees on how thick it is.
pub const FOCUS_RING_WIDTH: u32 = 2;

/// Builds the focus ring geometry for `rect` drawn with corner radius `radius`.
///
/// # Why a constructor rather than two calls at each draw site
///
/// Every control that shows keyboard focus needs the *same pair* — an inset rectangle
/// and a correspondingly tighter radius — and getting one of them wrong is invisible in
/// isolation (the ring simply hugs the corner slightly wrong). `ControlMetrics` owns the
/// geometry; this struct is what a draw site passes around so it cannot take one without
/// the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    /// The rectangle the ring's stroke follows.
    pub rect: Rect,
    /// The corner radius that rectangle should be drawn with.
    pub radius: u32,
}

impl FocusRing {
    /// The ring for a control occupying `rect` with corner radius `radius`.
    pub fn for_control(rect: Rect, radius: u32) -> Self {
        Self {
            rect: ControlMetrics::focus_ring_rect(rect),
            radius: ControlMetrics::focus_ring_radius(radius),
        }
    }

    /// Whether the ring has any area to paint.
    ///
    /// A control too small to hold two ring widths cannot show focus this way; the
    /// caller skips the stroke rather than painting a zero-sized one.
    pub fn is_drawable(&self) -> bool {
        self.rect.width > 0 && self.rect.height > 0
    }
}

/// The colour a focus ring takes.
///
/// # Why this is not the control's border colour
///
/// A ring must announce itself. Reading the same token a border reads is what made a
/// focused control indistinguishable from a merely bordered one (BLUE21 §七 B). This
/// function therefore takes the control's own colours and returns the contrast colour of
/// `fallback`'s surface, which is legible by construction on both appearances.
///
/// # Why there is no theme rung
///
/// The BLUE22 design called for a theme-provided `outline` token and this function used to
/// read `theme.colors.outline`. That field does not exist on [`Colors`], so the module never
/// compiled and the ring was unreachable code. Deriving the ring from the control's own ink
/// is the mechanism the crate actually has; a named `outline` token can be added to
/// [`Colors`] later, at which point this is the single site that reads it.
///
/// Takes the colour rather than a theme handle so this module stays free of the theme
/// layer and so a control with no theme can still ask.
pub fn focus_ring_color(fallback: crate::core::Color) -> crate::core::Color {
    fallback.contrast_color()
}

/// Device-independent control dimensions.
///
/// # Why these are constants and not per-control literals
///
/// Every value below is referenced by more than one control or by more than one
/// draw site within a control (a track's height and its corner radius, a
/// checkbox's box and its inset). Spreading them as literals meant the same fact
/// was derived in several places and could drift — the failure mode rule #101
/// names. The numbers themselves follow Flutter Material 3 and Qt Quick Basic,
/// which agree closely; where they differ the comment records both.
pub mod dimensions {
    use crate::core::Size;

    /// The smallest area a finger can reliably address on a touch device.
    ///
    /// Flutter's `kMinInteractiveDimension` (`material/constants.dart:27`).
    pub const TOUCH_TARGET_MIN: u32 = 48;

    /// A push button's minimum size: `64x40`.
    ///
    /// Flutter M3 `text_button.dart:555`. Qt's Basic `Button.qml:39-40` uses a
    /// roomier `100x40` floor; M3's is the tighter of the two and is what a
    /// desktop form wants.
    pub const BUTTON_MIN: Size = Size { width: 64, height: 40 };

    /// Horizontal padding of a push button's label: 12.
    pub const BUTTON_PADDING_H: u32 = 12;

    /// Vertical padding of a push button's label: 8.
    pub const BUTTON_PADDING_V: u32 = 8;

    /// Gap between a button's icon and its label: 6.
    pub const BUTTON_ICON_SPACING: u32 = 6;

    /// A button's icon box: 18 (Flutter M3 `text_button.dart:561`). Qt uses 24.
    pub const BUTTON_ICON_SIZE: u32 = 18;

    /// Corner radius of a rectangular button: 4 (Flutter M2 `text_button.dart:396`).
    pub const BUTTON_RADIUS: u32 = 4;

    /// A switch's track: `52x32`.
    ///
    /// Flutter M3 `switch.dart:2355-2379`. Qt's `Switch.qml:22-31` is `56x28`; M3's
    /// proportion (a 14 px thumb radius in a 32 px track) is the shape this crate
    /// draws.
    pub const SWITCH_TRACK: Size = Size { width: 52, height: 32 };

    /// A switch thumb's radius: 14, i.e. a 28 px disc in a 32 px track.
    pub const SWITCH_THUMB_RADIUS: u32 = 14;

    /// Inset of the switch thumb from the track edge: 2.
    pub const SWITCH_THUMB_INSET: u32 = 2;

    /// A checkbox indicator's box: `18x18` (Flutter `checkbox.dart:405`).
    pub const CHECKBOX_BOX: u32 = 18;

    /// A checkbox indicator's corner radius: 2.
    pub const CHECKBOX_RADIUS: u32 = 2;

    /// A checkbox tick / border stroke: 2.
    pub const CHECKBOX_STROKE: u32 = 2;

    /// A radio button's outer radius: 8 (Flutter `radio.dart:31`).
    ///
    /// An outer *diameter* of 16, matching the 18 px checkbox box closely enough
    /// that the two indicators read as a set.
    pub const RADIO_OUTER_RADIUS: u32 = 8;

    /// A radio button's inner dot radius: `4.5`, rounded to 5 (`radio.dart:32`).
    pub const RADIO_DOT_RADIUS: u32 = 5;

    /// A radio ring's stroke width: 2, matching [`CHECKBOX_STROKE`].
    pub const RADIO_STROKE: u32 = 2;

    /// The gap between an indicator and its label: 6.
    ///
    /// Qt's `CheckBox.qml:61` reads `spacing`, and that `spacing` means exactly
    /// this — indicator to text, never sibling to sibling (rule: `spacing` is not
    /// a sibling layout parameter).
    pub const INDICATOR_TEXT_SPACING: u32 = 6;

    /// A progress bar's height: 4 (Flutter M3 `progress_indicator.dart:1624`).
    pub const PROGRESS_HEIGHT: u32 = 4;

    /// A progress bar's corner radius: 2, i.e. fully rounded at 4 px thick.
    pub const PROGRESS_RADIUS: u32 = 2;

    /// A circular progress indicator's stroke width: 4.
    pub const PROGRESS_CIRCLE_STROKE: u32 = 4;

    /// A spinner's diameter: 48 — the size its own `size_hint` reports, so the two
    /// cannot describe different controls.
    ///
    /// A spinner is a **fixed-size indicator**, not a fraction of the area it is
    /// given. Deriving the diameter from `rect` (`min(w, h) / 2 * size_ratio`) drew a
    /// 90 px ring in the 240x120 census cell and a 36 px one in a 48 px row — the same
    /// control at two sizes, and the census image showed a circle filling most of the
    /// cell. Material's `CircularProgressIndicator` is 48 at its `medium` size, and
    /// that is also [`TOUCH_TARGET_MIN`], so an idle spinner is exactly one tap target.
    pub const SPINNER_DIAMETER: u32 = 48;

    /// The height of the star row a `rating` control draws: one 24 px star.
    ///
    /// A rating is a **row of fixed-size glyphs**, not a panel: the height is the
    /// star cell's own, so a 240x120 census cell gets the same row a 24 px list row
    /// gets instead of a 120 px-tall fill with five glyphs floating in the middle of
    /// it. It is the star size [`RATING_STAR_SIZE`] because a star's line box is the
    /// cell it needs.
    pub const RATING_ROW_HEIGHT: u32 = 24;

    /// The size of one star cell in a `rating` control: 24.
    ///
    /// The cell every star's advance and its [`RATING_STAR_GAP`] are measured from,
    /// so the stars are the same distance apart in any rectangle. Spreading them
    /// around the control's own centre made the pitch a function of the caller's
    /// width rather than of the star.
    pub const RATING_STAR_SIZE: u32 = 24;

    /// The gap between two star cells in a `rating` control: 4.
    pub const RATING_STAR_GAP: u32 = 4;

    /// A slider track's height: 4 (rounded from M2's 2 so the thumb reads as
    /// sitting *on* the track rather than floating above it).
    pub const SLIDER_TRACK_HEIGHT: u32 = 4;

    /// A slider track's corner radius: 2.
    pub const SLIDER_TRACK_RADIUS: u32 = 2;

    /// A slider thumb's radius: 10 (Flutter `slider_parts.dart:678`), i.e. a
    /// 20 px diameter thumb on a 4 px track.
    pub const SLIDER_THUMB_RADIUS: u32 = 10;

    /// A text field's content height floor: 48.
    ///
    /// Flutter's `kMinInteractiveDimension` (`input_decorator.dart:1116`); Qt's
    /// `TextField.qml:50` background is 40. A field is a tap target, so the
    /// touch-sized value wins.
    pub const TEXT_FIELD_MIN_HEIGHT: u32 = 48;

    /// A text field's horizontal content padding: 12.
    pub const TEXT_FIELD_PADDING_H: u32 = 12;

    /// A card's corner radius: 12 (Flutter `card.dart:322`).
    pub const CARD_RADIUS: u32 = 12;

    /// A dialog's corner radius: 28 (Flutter M3 `dialog.dart:1963-1966`).
    pub const DIALOG_RADIUS: u32 = 28;

    /// A dialog's minimum width: 280 (Flutter M3 `dialog.dart:275`).
    pub const DIALOG_MIN_WIDTH: u32 = 280;

    /// A dialog's content padding: 12 (Qt `Dialog.qml:21`).
    pub const DIALOG_PADDING: u32 = 12;

    /// The strip a dialog draws across its top to carry its title: 28.
    ///
    /// Eight dialog controls (`dialog`, `message_box`, `file_dialog`, `input_dialog`,
    /// `font_dialog`, `color_dialog`, `progress_dialog`, `find_replace_dialog`) each
    /// declared their own `TITLE_BAR_HEIGHT`/`28`, which is how the same visual object
    /// acquired several values (rule #101). It is the height that makes a 14 px title fit
    /// with [`DIALOG_PADDING`] above and below it.
    pub const DIALOG_TITLE_BAR_HEIGHT: u32 = 28;

    /// The height of a dialog's action-button row: 28.
    ///
    /// Shared with [`DIALOG_TITLE_BAR_HEIGHT`]'s reasoning: the OK/Cancel pair appears in
    /// every dialog above, and each spelled the button height itself.
    pub const DIALOG_BUTTON_HEIGHT: u32 = 28;

    /// A dialog's intrinsic height: 240 — the same as [`super::super::WidgetKind`]'s
    /// `Dialog`/`MessageBox` size hint family, and the cap a panel is centred at when it is
    /// given a taller area.
    pub const DIALOG_MIN_HEIGHT: u32 = 240;

    /// A toolbar's height: 56 (Flutter M3 `constants.dart:30`).
    pub const TOOLBAR_HEIGHT: u32 = 56;

    /// A toolbar's inter-item spacing: 6.
    pub const TOOLBAR_SPACING: u32 = 6;

    /// A toolbar item's edging inset from the bar's own edges: 2 on each side.
    ///
    /// The item band is `TOOLBAR_HEIGHT - 2 * TOOLBAR_ITEM_INSET` tall, which is what
    /// keeps a hover/checked fill from touching the strip's border. Named rather than
    /// repeated as `+ 2` / `- 4` at the three places that place an item (the draw path
    /// and the hit test both derive from `ToolBar::item_rect`).
    pub const TOOLBAR_ITEM_INSET: u32 = 2;

    /// A menu bar's height: 28 (Qt `QMenuBar` at 100% scale; Flutter M3's `AppBar`
    /// toolbar is 56, which is the *app bar*, not a menu bar's own `File Edit View`
    /// strip).
    ///
    /// One fact for both ends of a menu bar: the height it paints its band at, and the
    /// height a layout is told it wants. They were `28` in `size_hint` and `rect.height`
    /// in `draw`, so a 240x120 census cell drew a 120 px menu bar.
    pub const MENU_BAR_HEIGHT: u32 = 28;

    /// A status bar's height: 24 (Qt `QMainWindow`'s default status bar).
    ///
    /// The band `status_bar` paints and the height its `size_hint` reports, so the two
    /// cannot disagree about how thick the strip at the bottom of a window is.
    pub const STATUS_BAR_HEIGHT: u32 = 24;

    /// A tab's height in a tab strip (tab bar, tab widget, tab view): 24.
    ///
    /// One fact for five files. `tab_widget`, `tab_bar` and `tab_view` each carried their
    /// own `TAB_HEIGHT`/`40`/`24` literal, which is how the three tab controls came to draw
    /// three different tab heights for the same visual object (rule #101).
    pub const TAB_HEIGHT: u32 = 24;

    /// The narrowest a tab may become: 40 (`tabwidget`'s `MIN_TAB_WIDTH`).
    pub const TAB_MIN_WIDTH: u32 = 40;

    /// The widest a tab's *measured* width may become: 200 (its `MAX_TAB_WIDTH`).
    pub const TAB_MAX_WIDTH: u32 = 200;

    /// The gap between adjacent tabs: 2 (its `TAB_SPACING`).
    pub const TAB_SPACING: u32 = 2;

    /// The horizontal space a tab reserves for its own label: 24.
    ///
    /// `tabwidget`'s `TAB_TEXT_PADDING`: the label's advance is added to this before the
    /// result is clamped to `[TAB_MIN_WIDTH, TAB_MAX_WIDTH]`.
    pub const TAB_TEXT_PADDING: u32 = 24;

    /// A single-line page-navigation bar's height: 32 (Material's `Pagination` row).
    pub const PAGINATION_HEIGHT: u32 = 32;

    /// An app bar's height: 56 (Flutter M3 `constants.dart:30`, the same idea as
    /// [`TOOLBAR_HEIGHT`] — Material treats the app bar and the toolbar as one object).
    pub const APP_BAR_HEIGHT: u32 = 56;

    /// A bottom navigation bar's height: 56 (Flutter M3 `navigation_bar.dart:46`).
    pub const BOTTOM_NAV_HEIGHT: u32 = 56;

    /// A splitter handle's thickness: 5 (Qt `QSplitter`'s `handleWidth` default of 5 at
    /// 100% scale).
    ///
    /// It was `5` in `draw` and a second `HANDLE_WIDTH: f32 = 5.0` in `begin_handle_drag`,
    /// so the band the user sees and the band the pointer must hit could drift apart.
    pub const SPLITTER_HANDLE_THICKNESS: u32 = 5;

    /// The strip a menu draws above its popup body to carry its title: 20.
    ///
    /// A closed menu still paints this strip, which is what makes the control visible at
    /// rest (see `Menu::draw`).
    pub const MENU_HEADING_HEIGHT: u32 = 20;

    /// A menu popup's body padding, above the first entry row and below the last: 2.
    pub const MENU_POPUP_PADDING: u32 = 2;

    /// A menu separator's row height: 6 (the rule sits on the row's centre line, so the
    /// space above and below it is `(MENU_SEPARATOR_HEIGHT - DIVIDER_THICKNESS) / 2`).
    pub const MENU_SEPARATOR_HEIGHT: u32 = 6;

    /// A page/tab content header's minimum height: 24.
    ///
    /// `collapsible_pane`'s header and `toolbox`'s tab share this "one line of chrome I can
    /// click" shape.
    pub const PANE_HEADER_HEIGHT: u32 = 24;

    /// A scrollbar's thickness: 8 (Flutter `scrollbar.dart:12-16`).
    pub const SCROLLBAR_THICKNESS: u32 = 8;

    /// A scrollbar thumb's minimum length: 48. A proportional thumb with no floor
    /// disappears on a very long document.
    pub const SCROLLBAR_MIN_LENGTH: u32 = 48;

    /// A horizontal divider's thickness: 1.
    pub const DIVIDER_THICKNESS: u32 = 1;

    /// The vertical space a divider reserves: 16 (Flutter `divider.dart:360-365`).
    pub const DIVIDER_SPACING: u32 = 16;

    /// A tooltip's box height: 24 (Flutter desktop `tooltip.dart:425-448`).
    pub const TOOLTIP_HEIGHT: u32 = 24;

    /// A tooltip's horizontal padding: 8.
    pub const TOOLTIP_PADDING_H: u32 = 8;

    /// A tooltip's vertical padding: 4.
    pub const TOOLTIP_PADDING_V: u32 = 4;

    /// The base font size: 14 (Flutter `text_painter.dart:42`).
    pub const FONT_SIZE_BASE: u32 = 14;

    /// The diameter (or side) of a `avatar` control: 40, the size its own `new`
    /// falls back to for a degenerate rectangle and the size its `size_hint`
    /// reports.
    ///
    /// An avatar is a **fixed-size disc**, not a fraction of the area it is given.
    /// Deriving it from `rect` and anchoring it at the rectangle's top-left corner
    /// drew a half-clipped circle in the 240x120 census cell (`circle cx=60 cy=60
    /// r=60`, i.e. running from x 0 to x 120 with the left half of the cell empty).
    /// One fact for the constructor's fallback, the hint a layout reads and the
    /// centred disc the draw path paints, so all three describe the same avatar.
    pub const AVATAR_SIZE: u32 = 40;

    /// A chip's height: 32 (Flutter M3 `chip.dart`, `_kChipHeight`).
    ///
    /// A chip is chrome: it is the same height whoever hands it the row, so a 240x120
    /// census cell must not draw a 112 px chip. `chip` and every list that hosts a chip
    /// read this one value.
    pub const CHIP_HEIGHT: u32 = 32;

    /// A chip's horizontal label padding: 8 on each side.
    pub const CHIP_PADDING_H: u32 = 8;

    /// A segmented control's track height: 32.
    ///
    /// One fact for three controls — `segmented_control`, `segmented_button` and
    /// `cupertino_segmented_control` each declared `32` in their own `size_hint`, and each
    /// painted the track at `rect.height` instead, so the drawn pill and the reported size
    /// disagreed on every one of them.
    pub const SEGMENTED_CONTROL_HEIGHT: u32 = 32;

    /// One row of a wheel picker: 32.
    ///
    /// A date picker's drum is a *fixed row* list, not a fraction of its area. Deriving the
    /// row from `rect.height / 5` made a 120 px cell show five 24 px rows and a 300 px panel
    /// five 60 px rows — the same control at two different densities. iOS `UIPickerView`'s
    /// date mode uses 32 pt rows at every container size.
    pub const PICKER_ROW_HEIGHT: u32 = 32;

    /// The number of rows a wheel picker keeps visible: 5 (the selection plus two either
    /// side), which is what `UIPickerView` shows at rest.
    pub const PICKER_VISIBLE_ROWS: u32 = 5;

    /// A Cupertino navigation bar's compact height: 44 (`UINavigationBar`'s standard
    /// height).
    pub const NAV_BAR_HEIGHT: u32 = 44;

    /// A Cupertino navigation bar's large-title height: 96.
    ///
    /// The large-title bar is the compact bar plus the title's own row, which is the shape
    /// `UINavigationBar` adopts when `prefersLargeTitles` is set.
    pub const NAV_BAR_LARGE_HEIGHT: u32 = 96;

    /// A browser URL bar's height: 28 (Chrome's toolbar strip at 100 % zoom).
    pub const WEB_URL_BAR_HEIGHT: u32 = 28;

    /// A breadcrumb trail's height: 28 (one line of 14 px links plus 7 px of air above
    /// and below).
    ///
    /// It matches [`SPLIT_BUTTON_HEIGHT`] because both are "one compact row of chrome".
    pub const BREADCRUMB_HEIGHT: u32 = 28;

    /// A split button's height: 28, the same compact row as [`BREADCRUMB_HEIGHT`].
    pub const SPLIT_BUTTON_HEIGHT: u32 = 28;

    /// The trailing arrow column's width: 22.
    ///
    /// # Why this is a constant and not derived from the glyph
    ///
    /// BLUE22 §B.9 asks a sub-part's box to be derived from its sibling, and this is the one
    /// place in `split_button` where the derivation is **the constant itself**: the column is
    /// the arrow's own width, so the trigger's width is whatever the column leaves. The two
    /// boxes therefore tile the band — a wider column *pushes* the trigger narrower instead of
    /// overlapping it — which is the property that matters. Deriving the number from the 'v'
    /// glyph's advance instead would tie the column to one character's metric in one font, and
    /// a 22 px column holding a 8 px glyph is deliberately roomier than the glyph (a 8 px target
    /// is not a target).
    ///
    /// It is a named constant rather than the field initialiser it used to be so the hit test,
    /// the separator line and the band derivation all read one number (rule #101).
    pub const SPLIT_ARROW_COLUMN_WIDTH: u32 = 22;

    /// Horizontal padding of a split button's label and a menu row's label: 8.
    ///
    /// Qt Basic's `Button.qml` uses `padding: 6` on a compact control and `MenuItem.qml` uses
    /// `padding: 6` with a `leftPadding` that adds the indicator. 8 is the crate's existing
    /// value for both, kept as a name so the label and the menu rows cannot drift apart — they
    /// were two independent `x + 8` literals.
    pub const SPLIT_BUTTON_PADDING_H: u32 = 8;

    /// A menu row's leading inset: 8, the same compact row padding Qt's `MenuItem.qml` uses.
    pub const MENU_ROW_PADDING_H: u32 = 8;

    /// The width a menu row reserves for its shortcut and its submenu arrow: 28.
    ///
    /// One number for both trailing affordances, because they are drawn in one column area: a
    /// row shows at most one of them at a time (a submenu entry's shortcut is not useful), so
    /// two separate reserves would only mean two numbers to keep in step. It is a constant so
    /// the label's box and the arrow's origin read the same value (rule #101).
    pub const MENU_ROW_TRAILING_WIDTH: u32 = 28;

    /// A menu row's height: 22 (one 14 px line plus 4 px of air above and below).
    pub const MENU_ROW_HEIGHT: u32 = 22;

    /// A status bar's horizontal padding: 6.
    ///
    /// The distance from the strip's own edge to its first message, and the distance the size
    /// grip keeps from the strip's corner. One number for both, because they are the same fact:
    /// how far this control's content sits from its own edge. The grip's box and the message's
    /// inset were previously two unrelated literals (`- 14` and `+ 6`/`+ 12`), so the room
    /// reserved for the grip and the room it used could not be kept in agreement.
    pub const STATUS_BAR_PADDING_H: u32 = 6;

    /// A status bar's size grip: 12 x 12, the classic three-diagonal resize affordance.
    ///
    /// Also the answer to "how far from the corner does the grip sit", because the reserve the
    /// permanent message leaves is derived from this box.
    pub const STATUS_GRIP_SIZE: u32 = 12;

    /// A snackbar's height: 48 (Material's single-line snackbar).
    pub const SNACKBAR_HEIGHT: u32 = 48;

    /// A toast's height: 48, the snackbar's own bar height.
    pub const TOAST_HEIGHT: u32 = 48;

    /// A toast's leading accent stripe: 4 wide, running the toast's own height.
    pub const TOAST_ACCENT_WIDTH: u32 = 4;

    /// The label column a timeline or Gantt chart reserves at its left edge: 120.
    ///
    /// One fact for two charts. They carried `120`/`130` and `150`/`160` as four literals —
    /// two per file, one for the text and one for the track — so each chart's track began at
    /// a different x from the label gutter it belonged to, and the two charts disagreed about
    /// how wide a task name column is.
    pub const CHART_LABEL_GUTTER: i32 = 120;

    /// A timeline or Gantt chart's trailing margin after its track: 10.
    pub const CHART_TRACK_MARGIN: i32 = 10;

    /// A video player's transport bar height: 36.
    ///
    /// A fixed overlay strip pinned to the bottom of the video surface, not a fraction of it.
    pub const VIDEO_CONTROL_BAR_HEIGHT: u32 = 36;

    /// A video player's seek bar height: 8, the scrollbar's own thickness.
    pub const VIDEO_SEEK_BAR_HEIGHT: u32 = 8;

    /// The number of rows a `roller` wheel keeps visible at once: 5 (the selection
    /// plus two either side), the same reading as [`PICKER_VISIBLE_ROWS`].
    ///
    /// The roller's `visible_count` is already clamped to 5, so this names the height
    /// the wheel occupies. Sizing the wheel from the control's own rectangle instead
    /// drew a 120 px-tall fill in the census cell — a surface, not a wheel.
    pub const ROLLER_VISIBLE_ROWS: u32 = 5;

    /// The height of one `roller` row: 28.
    ///
    /// A wheel's rows are a **fixed row** list, like a picker's drum: deriving the
    /// row from the visible count and the control's height made the same control show
    /// 24 px rows in a short box and 60 px rows in a tall one. 28 is the row the
    /// control's own default 16 pt face needs — see `Roller::item_height`, which this
    /// is the base of — and it is the same row `emoji_picker` uses at the same size.
    pub const ROLLER_ROW_HEIGHT: u32 = 28;

    /// The height of a `roller`'s wheel: five rows, the [`PICKER_VISIBLE_ROWS`] a
    /// picker's drum shows.
    ///
    /// A wheel's whole extent is therefore 140 px, which is why a wheel in the
    /// 240x120 census cell is clamped to the cell: the control fills the rectangle it
    /// is given while it is smaller than that, and is centred when it is not. Sizing
    /// the wheel from the control's own rectangle instead drew the census cell as a
    /// 120 px-tall surface with a single 24 px selection band in it — a panel, not a
    /// wheel.
    pub const ROLLER_WHEEL_HEIGHT: u32 = ROLLER_VISIBLE_ROWS * ROLLER_ROW_HEIGHT;

    /// The height of a `stepper`'s increment/decrement row: 48.
    ///
    /// The row the +/− buttons and the value live in, so all three are centred on the
    /// control's middle line together. The buttons were painted `rect.height - 2`
    /// tall, which made a 240x120 census cell draw **118 px buttons** — two full-height
    /// slabs with a number between them rather than a stepper.
    pub const STEPPER_ROW_HEIGHT: u32 = TOUCH_TARGET_MIN;

    /// The width of one `stepper` button: 48.
    ///
    /// A square button, so it is the same object in a wide row as in a narrow one.
    /// The old `rect.height.min(rect.width / 3).max(20)` read the *control's* height,
    /// which is why the buttons stretched with the cell.
    pub const STEPPER_BUTTON_WIDTH: u32 = TOUCH_TARGET_MIN;

    /// The padding a `stepper` leaves between its own edge and its row: 2.
    pub const STEPPER_PADDING: u32 = 2;

    /// The height of a `search_box`'s field: 48, the same value [`TEXT_FIELD_MIN_HEIGHT`]
    /// names for every other text entry control.
    ///
    /// A search box is a text field with a magnifier in it, so it must be the same
    /// height as one: this control drew `rect.height` (120 px in the census cell) while
    /// `lineedit` drew a 48 px band, so the two entry controls in the same form were
    /// different objects.
    pub const SEARCH_BOX_FIELD_HEIGHT: u32 = TEXT_FIELD_MIN_HEIGHT;

    /// A `badge` pill's height: 18, the value its own geometry derivation already
    /// clamps to; naming it makes the pill's box and its label's line box come from
    /// one fact.
    pub const BADGE_PILL_HEIGHT: u32 = 18;

    /// A `badge` pill's horizontal label padding: 6 on each side.
    pub const BADGE_PILL_PADDING_H: u32 = 6;

    /// A `badge` pill's vertical label padding: 2 above and below.
    ///
    /// Small on purpose: a badge is a compact marker, so the pill wraps its label
    /// closely. Named rather than written as `* 2` at the draw site because the pill's
    /// height is derived from it, and a bare `2` there is the same constant the old
    /// `padding_y` was.
    pub const BADGE_PILL_PADDING_V: u32 = 2;

    /// The label font size a `badge` draws its count in: 11 (Material's `labelSmall`).
    pub const BADGE_LABEL_FONT_SIZE: u32 = 11;

    /// A `skeleton_loader` placeholder row's height: 20.
    ///
    /// One shimmer line is a fixed-height row, not a fraction of the control — the
    /// `Rect` shape's own `(w, h)` is the caller's datum, but the `TextLine` shape
    /// stacks rows of *this* height, so a three-line placeholder is 76 px tall in any
    /// rectangle.
    pub const SKELETON_ROW_HEIGHT: u32 = 20;

    /// The gap between two `skeleton_loader` placeholder rows: 8.
    pub const SKELETON_ROW_GAP: u32 = 8;

    /// A `color_well`'s swatch size: 60, the sample a colour control exists to show.
    ///
    /// The well is a **fixed-size affordance**, not a panel: sizing the checkerboard
    /// and the swatch from the control's rectangle drew a 240x120 chequerboard whose
    /// swatch covered most of the cell (`color_well.svg` was 1808 rects). Flutter's
    /// `ColorWell` sample is 60 px square at its default density.
    pub const COLOR_WELL_SIZE: u32 = 60;

    /// The number of swatches a `color_history` lays out per row: 5.
    ///
    /// The grid is measured from the swatch's own size, so the panel is
    /// `5 * (20 + 4)` wide in any rectangle rather than as many swatches as the
    /// caller's width happens to fit.
    pub const COLOR_HISTORY_PER_ROW: u32 = 5;

    /// One `color_history` swatch: 20x20.
    pub const COLOR_HISTORY_SWATCH: u32 = 20;

    /// The gap between two `color_history` swatches: 4.
    pub const COLOR_HISTORY_PADDING: u32 = 4;

    /// The height of one `color_history` row: 20, the swatch's own box.
    pub const COLOR_HISTORY_ROW_HEIGHT: u32 = COLOR_HISTORY_SWATCH;

    /// A `refresh_control`'s pull indicator height: 40.
    ///
    /// The reveal panel is a **fixed strip** at the top of the control, so a pull in
    /// a 400 px list and a pull in the census cell open the same 40 px band. It was
    /// `40` when refreshing and `0` otherwise, with the control's own rectangle
    /// stretching beneath it.
    pub const REFRESH_INDICATOR_HEIGHT: u32 = 40;

    /// The step between visual-density levels: 4 logical px per unit
    /// (`theme_data.dart:3307-3314`).
    pub const DENSITY_STEP: u32 = 4;

    /// Which way a visual-density level shifts metric sizes.
    ///
    /// Flutter's `VisualDensity` is a signed offset in [`DENSITY_STEP`] units, and
    /// Material's named levels are `comfortable = -4` and `compact = -8`. Only the
    /// *vertical* metrics shrink at the compact end — Flutter deliberately does
    /// **not** compress horizontal padding (`button_style_button.dart:502`),
    /// because a compact desktop button with 0 px of side padding stops reading as
    /// a button.
    pub fn density_scale(vertical_density: i32) -> i32 {
        vertical_density.saturating_mul(DENSITY_STEP as i32)
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::style::EdgeOffsets;

    #[test]
    fn small_content_still_gets_the_floor() {
        // 5 px of text in a button: the background floor wins. This is the
        // assertion behind "a control must not shrink below what a finger can
        // address", and it is the regression guard for the 240x120 stadium.
        let size = ControlMetrics::implicit_size(
            Size::new(5, 5),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, dimensions::BUTTON_MIN);
    }

    #[test]
    fn large_content_wins_over_the_floor() {
        // Content + padding exceeds the floor on both axes, so it decides.
        let size = ControlMetrics::implicit_size(
            Size::new(200, 60),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, Size::new(224, 76));
    }

    #[test]
    fn the_two_arms_are_compared_component_wise() {
        // Content loses on width but wins on height: each axis decides alone.
        let size = ControlMetrics::implicit_size(
            Size::new(10, 90),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, Size::new(64, 106));
    }

    #[test]
    fn content_box_removes_padding_from_both_sides() {
        let rect = Rect::new(10, 20, 100, 50);
        let content = ControlMetrics::content_box(rect, EdgeOffsets::symmetric(4, 6));
        assert_eq!(content, Rect::new(16, 24, 88, 42));
    }

    #[test]
    fn a_panel_is_capped_at_its_intrinsic_size_and_centred() {
        // The census case: a 240x120 cell offered to a panel whose intrinsic size is
        // 280x240. The width cannot exceed the cell, so the panel is the whole width; the
        // height is capped at the intrinsic 240 (which is larger than the cell) so the
        // panel is the whole height too. Neither axis is stretched beyond what the panel
        // asked for, which is the ``switch``-shaped-rectangle defect inverted.
        let cell = crate::widget::census::CENSUS_RECT;
        let panel = ControlMetrics::painted_box(
            cell,
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        );
        assert_eq!(panel, Rect::new(0, 0, 240, 120));

        // Given *more* room than its intrinsic size, the panel keeps that size and is
        // centred, so the frame reads as a dialog rather than as a wall of fill.
        let roomy = Rect::new(0, 0, 400, 300);
        let centred = ControlMetrics::painted_box(
            roomy,
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        );
        assert_eq!(centred, Rect::new(60, 30, 280, 240));
    }

    #[test]
    fn a_panel_never_collapses_to_zero_extent() {
        // A zero-extent rectangle is an invisible element, which is a defect rather than a
        // tight fit: a panel squeezed to nothing has to keep one pixel of itself visible.
        let degenerate = Rect::new(5, 5, 0, 0);
        let panel = ControlMetrics::painted_box(degenerate, Size::new(280, 240));
        assert_eq!(panel.width, 1);
        assert_eq!(panel.height, 1);
    }

    #[test]
    fn oversized_padding_collapses_the_content_box_instead_of_inverting_it() {
        let rect = Rect::new(0, 0, 10, 10);
        let content = ControlMetrics::content_box(rect, EdgeOffsets::all(50));
        assert_eq!(content.width, 0);
        assert_eq!(content.height, 0);
        assert_eq!(content.x, 50);
        assert_eq!(content.y, 50);
    }

    #[test]
    fn centered_chrome_is_centred_and_clamped_never_expanded() {
        // Roomier than the chrome: centred, chrome keeps its own size.
        let roomy = ControlMetrics::center_in(Rect::new(0, 0, 240, 120), dimensions::SWITCH_TRACK);
        assert_eq!(roomy, Rect::new(94, 44, 52, 32));

        // Smaller than the chrome: clamped to the available area, not expanded.
        let tight = ControlMetrics::center_in(Rect::new(5, 7, 20, 10), dimensions::SWITCH_TRACK);
        assert_eq!(tight, Rect::new(5, 7, 20, 10));
    }

    #[test]
    fn a_disc_stays_square_in_a_non_square_area() {
        let disc = ControlMetrics::centered_disc(Rect::new(0, 0, 100, 20), 18);
        assert_eq!(disc.width, disc.height);
        assert_eq!(disc, Rect::new(41, 1, 18, 18));
    }

    #[test]
    fn a_leading_box_sits_at_the_padding_edge_and_never_leaves_the_rect() {
        // A checkbox indicator: 18x18, 2 px in from the left, centred vertically.
        let indicator = ControlMetrics::leading_box(
            Rect::new(0, 0, 100, 40),
            Size::new(dimensions::CHECKBOX_BOX, dimensions::CHECKBOX_BOX),
            EdgeOffsets::all(2),
        );
        assert_eq!(indicator, Rect::new(2, 11, 18, 18));

        // Padding so large the box would fall out: it is pinned to the right edge
        // of the rectangle instead of being painted past it.
        let pinned = ControlMetrics::leading_box(
            Rect::new(0, 0, 20, 40),
            Size::new(dimensions::CHECKBOX_BOX, dimensions::CHECKBOX_BOX),
            EdgeOffsets::all(50),
        );
        assert_eq!(pinned.x, 2);
        assert_eq!(pinned.x + pinned.width as i32, 20);
    }

    #[test]
    fn a_band_is_centred_and_keeps_its_thickness() {
        // The whole point: a 4 px progress bar in a 120 px cell stays 4 px.
        let band =
            ControlMetrics::centered_band(Rect::new(0, 0, 240, 120), dimensions::PROGRESS_HEIGHT);
        assert_eq!(band, Rect::new(0, 58, 240, 4));

        // A band thicker than the rectangle is clamped to it.
        let clamped = ControlMetrics::centered_band(Rect::new(0, 10, 100, 2), 4);
        assert_eq!(clamped, Rect::new(0, 10, 100, 2));
    }

    /// The metric table's own invariants, extended with the constants added for the
    /// controls that used to derive their chrome from `rect`.
    ///
    /// Each of these is a "one fact, two consumers" value: a diameter that must agree
    /// with a `size_hint`, a row that must be its own height, a padding that must not
    /// vanish. Asserting the relation here means a later edit to one of the pair cannot
    /// silently break it in a control whose snapshot nobody re-read.
    /// The dimensions table is self-consistent, checked **at compile time**.
    ///
    /// # Why these are `const` assertions rather than ordinary ones
    ///
    /// Every claim below relates two constants to each other — a thumb's diameter against
    /// its track, a pill's height against its label's line box, a field's height against
    /// the touch floor. They are properties of the table rather than of any execution, so
    /// stating them in a `const` block makes a contradictory pair a **build failure** rather
    /// than a test failure. A test can be filtered out, or run against a stale binary; a
    /// `const` assertion cannot, which is exactly the guarantee a table like this needs.
    #[test]
    fn the_dimensions_table_is_internally_consistent() {
        const {
            // ── Sliders: the handle is wider than its groove ──
            // A handle no wider than the track it rides on is a line, not a grip.
            assert!(dimensions::SLIDER_THUMB_RADIUS * 2 > dimensions::SLIDER_TRACK_HEIGHT);
            assert!(dimensions::SLIDER_TRACK_RADIUS * 2 <= dimensions::SLIDER_TRACK_HEIGHT);

            // ── The switch: thumb plus insets exactly fill the track's height ──
            assert!(
                dimensions::SWITCH_THUMB_RADIUS * 2 + dimensions::SWITCH_THUMB_INSET * 2
                    == dimensions::SWITCH_TRACK.height
            );
            assert!(dimensions::SWITCH_THUMB_RADIUS * 2 < dimensions::SWITCH_TRACK.width);

            // ── The bar family is a stadium at its own height ──
            assert!(dimensions::PROGRESS_RADIUS * 2 == dimensions::PROGRESS_HEIGHT);
            assert!(dimensions::PROGRESS_HEIGHT <= dimensions::SLIDER_TRACK_HEIGHT * 2);

            // ── A scrollbar's floor is longer than its trough is thick ──
            // A proportional thumb shrunk to the floor must still be a grabbable grip
            // rather than a dot.
            assert!(dimensions::SCROLLBAR_MIN_LENGTH > dimensions::SCROLLBAR_THICKNESS);

            // ── The indicator pair reads as a set ──
            // A checkbox box and a radio ring sit side by side in every form; letting them
            // differ by more than a couple of pixels makes them look like different control
            // families rather than two spellings of one choice.
            assert!(dimensions::CHECKBOX_BOX >= dimensions::RADIO_OUTER_RADIUS * 2);
            assert!(dimensions::CHECKBOX_BOX - dimensions::RADIO_OUTER_RADIUS * 2 <= 2);
            assert!(dimensions::RADIO_DOT_RADIUS < dimensions::RADIO_OUTER_RADIUS);

            // ── The row-like controls are bands, not panels ──
            assert!(dimensions::MENU_BAR_HEIGHT < dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::STATUS_BAR_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::TAB_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::PAGINATION_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::BREADCRUMB_HEIGHT <= dimensions::TOOLBAR_HEIGHT);

            // ── A stepper's buttons fit inside its row ──
            assert!(dimensions::STEPPER_BUTTON_WIDTH <= dimensions::STEPPER_ROW_HEIGHT);
            assert!(dimensions::STEPPER_ROW_HEIGHT - dimensions::STEPPER_PADDING * 2 > 0);

            // ── The touch floor is the strongest floor ──
            // A field is the smallest control a finger must address, and a button is
            // taller than the floor rather than equal to it.
            assert!(dimensions::TEXT_FIELD_MIN_HEIGHT >= dimensions::TOUCH_TARGET_MIN);
            assert!(dimensions::TOUCH_TARGET_MIN > dimensions::BUTTON_MIN.height);

            // ── A search box is a text field, by construction ──
            assert!(dimensions::SEARCH_BOX_FIELD_HEIGHT == dimensions::TEXT_FIELD_MIN_HEIGHT);

            // ── A badge's pill holds its own label's line box ──
            assert!(dimensions::BADGE_PILL_HEIGHT > dimensions::BADGE_LABEL_FONT_SIZE);

            // ── A color history row is the swatch it holds ──
            assert!(dimensions::COLOR_HISTORY_ROW_HEIGHT == dimensions::COLOR_HISTORY_SWATCH);
            assert!(dimensions::COLOR_HISTORY_PER_ROW > 0);

            // ── A skeleton's gap cannot consume its own row ──
            assert!(dimensions::SKELETON_ROW_GAP < dimensions::SKELETON_ROW_HEIGHT);

            // ── The refresh indicator is a strip, not the whole control ──
            assert!(dimensions::REFRESH_INDICATOR_HEIGHT < 120);

            // ── The roller shows the same number of rows a picker does ──
            assert!(dimensions::ROLLER_VISIBLE_ROWS == dimensions::PICKER_VISIBLE_ROWS);
            assert!(
                dimensions::ROLLER_WHEEL_HEIGHT
                    == dimensions::ROLLER_VISIBLE_ROWS * dimensions::ROLLER_ROW_HEIGHT
            );
        }
    }

    #[test]
    fn mirrored_offsets_exchange_horizontal_sides_only() {
        let offsets = EdgeOffsets::new(1, 2, 3, 4);
        let mirrored = offsets.mirrored();
        assert_eq!(mirrored.top, 1);
        assert_eq!(mirrored.bottom, 3);
        assert_eq!(mirrored.left, 2);
        assert_eq!(mirrored.right, 4);
        assert_eq!(mirrored.horizontal_total(), offsets.horizontal_total());
    }

    #[test]
    fn a_focus_ring_is_inset_so_it_never_leaves_the_control() {
        // The ring must fit inside the control: a control laid out flush against a
        // neighbour must not paint over it.
        let ring = ControlMetrics::focus_ring_rect(Rect::new(0, 0, 64, 40));
        assert_eq!(ring, Rect::new(2, 2, 60, 36));
        assert!(ring.x >= 0 && ring.y >= 0);
        assert!(ring.x + ring.width as i32 <= 64);
        assert!(ring.y + ring.height as i32 <= 40);

        // A control smaller than two ring widths collapses to zero rather than
        // inverting, the same rule `content_box` follows.
        let tiny = ControlMetrics::focus_ring_rect(Rect::new(5, 5, 2, 2));
        assert_eq!(tiny.width, 0);
        assert_eq!(tiny.height, 0);
    }

    #[test]
    fn a_focus_ring_follows_the_control_corner_tightly() {
        // A stadium stays a stadium; square corners stay square.
        assert_eq!(ControlMetrics::focus_ring_radius(0), 0);
        assert_eq!(ControlMetrics::focus_ring_radius(6), 4);
        // A radius smaller than the ring cannot produce one, so it floors at 0.
        assert_eq!(ControlMetrics::focus_ring_radius(1), 0);
    }

    #[test]
    fn horizontal_and_vertical_totals_add_both_sides() {
        let offsets = EdgeOffsets::new(3, 5, 7, 11);
        assert_eq!(offsets.horizontal_total(), 16);
        assert_eq!(offsets.vertical_total(), 10);
    }

    #[test]
    fn a_top_band_keeps_its_thickness_at_the_top_edge() {
        // A nav bar in a 120 px census cell: 44 px at y 0, not 44 px centred and not
        // 120 px tall. This is the assertion behind `pagination.svg` and `app_bar.svg`
        // no longer drawing a full-canvas chrome bar.
        let band = ControlMetrics::top_band(Rect::new(0, 0, 240, 120), dimensions::TAB_HEIGHT);
        assert_eq!(band, Rect::new(0, 0, 240, 24));

        // A band taller than the rectangle is clamped to it rather than painted past it.
        let clamped = ControlMetrics::top_band(Rect::new(5, 7, 100, 10), 40);
        assert_eq!(clamped, Rect::new(5, 7, 100, 10));
    }

    #[test]
    fn a_bottom_band_keeps_its_thickness_at_the_bottom_edge() {
        // A bottom tab strip in a 120 px cell: its height sits on the bottom edge, so
        // `TabWidget`'s South tabs are at y 96 and not y -24 as they were before.
        let band = ControlMetrics::bottom_band(Rect::new(0, 0, 240, 120), dimensions::TAB_HEIGHT);
        assert_eq!(band, Rect::new(0, 96, 240, 24));
        assert_eq!(band.y + band.height as i32, 120, "the strip must end on the control's edge");
    }

    #[test]
    fn a_bands_inset_is_measured_from_the_band_not_the_control() {
        // A toolbar item: 2 px of edging from the 56 px strip it sits in.
        let strip = ControlMetrics::top_band(Rect::new(0, 0, 240, 120), dimensions::TOOLBAR_HEIGHT);
        let item = ControlMetrics::band_inset(strip, dimensions::TOOLBAR_ITEM_INSET);
        assert_eq!(item, Rect::new(2, 2, 236, 52));

        // An inset that would invert the band collapses it instead, the same rule
        // `content_box` follows.
        let collapsed = ControlMetrics::band_inset(Rect::new(4, 4, 6, 6), 10);
        assert_eq!(collapsed.width, 0);
        assert_eq!(collapsed.height, 0);
    }

    #[test]
    fn a_top_band_and_the_content_below_it_tile_the_rectangle() {
        // The two are derived from one height, so they cannot overlap or leave a gap:
        // a nav bar that covered its own page, or floated above it, is what this pins.
        let rect = Rect::new(0, 0, 240, 120);
        let band = ControlMetrics::top_band(rect, dimensions::TAB_HEIGHT);
        let content = ControlMetrics::content_below_top_band(rect, dimensions::TAB_HEIGHT);
        assert_eq!(content.y, band.y + band.height as i32);
        assert_eq!(content.height + band.height, rect.height);
        assert_eq!(content, Rect::new(0, 24, 240, 96));
    }

    #[test]
    fn a_bottom_band_and_the_content_above_it_tile_the_rectangle() {
        let rect = Rect::new(0, 0, 240, 120);
        let band = ControlMetrics::bottom_band(rect, dimensions::TAB_HEIGHT);
        let content = ControlMetrics::content_above_bottom_band(rect, dimensions::TAB_HEIGHT);
        assert_eq!(content.y + content.height as i32, band.y);
        assert_eq!(content.height + band.height, rect.height);
        assert_eq!(content, Rect::new(0, 0, 240, 96));
    }

    #[test]
    fn a_strip_taller_than_the_control_leaves_no_content_rather_than_a_negative_one() {
        let rect = Rect::new(10, 20, 100, 10);
        assert_eq!(ControlMetrics::content_below_top_band(rect, 40).height, 0);
        assert_eq!(ControlMetrics::content_above_bottom_band(rect, 40).height, 0);
    }
}
