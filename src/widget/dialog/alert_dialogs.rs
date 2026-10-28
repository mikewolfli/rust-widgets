// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `AlertDialog`, `ConfirmDialog` and `AboutDialog` — the three modal notices a form needs.
//!
//! # Three controls, one card
//!
//! All three draw the same card: a severity glyph beside the title, a wrapped message, and a
//! right-aligned action row. What differs is the *action set* and therefore the events, and the
//! severity the glyph announces:
//!
//! | control | actions | events | glyph |
//! |---|---|---|---|
//! | [`AlertDialog`] | one acknowledgement | `accepted` | warning triangle |
//! | [`ConfirmDialog`] | accept / reject | `accepted`, `rejected` | question mark |
//! | [`AboutDialog`] | one dismissal | `closed` | information mark |
//!
//! They are three capabilities and three kinds rather than one control with a mode flag,
//! because the *published contract* differs: a designer wiring an `AlertDialog` must not be
//! offered a `rejected` wire that the control can never fire.
//!
//! # Where the card is painted
//!
//! Inside the control's own geometry. Placement — centring over the window, the scrim, the
//! modal grab — belongs to the host, which is why none of these controls moves its own
//! rectangle. The shared card painter lives in this module because all three use it; a second
//! copy would be the "two answers to one question" defect.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver, SemanticColor};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// The padding inside the card, in logical pixels.
const CARD_PADDING: u32 = 16;
/// The corner radius of the card.
const CARD_RADIUS: u32 = 8;
/// The height of one action button.
const BUTTON_HEIGHT: u32 = 28;
/// The gap between two action buttons.
const BUTTON_GAP: u32 = 8;
/// The horizontal padding inside a button.
const BUTTON_PADDING: u32 = 12;
/// The distance between the title's line box and the message's.
const TITLE_GAP: u32 = 4;
/// The gutter between a card's severity glyph and the text that starts after it.
const ICON_TITLE_GAP: u32 = 8;

/// The card's resolved colours and font.
struct CardPalette {
    surface: Color,
    ink: Color,
    accent: Color,
    border: Color,
    font: Font,
    title_font: Font,
}

/// Resolves the card palette from the control's style and the active theme.
///
/// The palette is read from the shared snapshot once per draw, so no manager lock is held
/// across the drawing calls.
fn card_palette(base: &BaseWidget, role: &str) -> CardPalette {
    let style = base.style();
    let themed = crate::style::resolved_theme_style_shared(role);
    let (window_background, window_foreground, accent, outline) = {
        match crate::style::palette_shared() {
            Some(colors) => (colors.background, colors.foreground, colors.primary, colors.outline),
            None => (Color::WHITE, Color::BLACK, Color::PRIMARY, Color::rgb(190, 190, 190)),
        }
    };
    let surface = style
        .background_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
        .unwrap_or(window_background);
    let ink = style
        .text_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
        .unwrap_or(window_foreground);
    let accent = themed.as_ref().and_then(|resolved| resolved.border_color).unwrap_or(accent);
    let border = style
        .border_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.border_color))
        .unwrap_or(outline);
    let font = style
        .font
        .clone()
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.font.clone()))
        .unwrap_or_else(|| Font::simple("Arial", 12.0));
    let title_font = Font::simple(font.family(), font.size() + 2.0);
    CardPalette { surface, ink, accent, border, font, title_font }
}

/// The severity glyph a dialog card draws beside its title.
///
/// Each dialog fixes its own icon, so unlike the message box there is no icon property to set:
/// an alert warns, a confirmation asks, an about card informs. The glyphs are the shared
/// [`IconName`] outlines, not text symbols — `⚠`/`✗` are covered by no bundled face, so drawing
/// them through the text pipeline fell back to bitmap blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardIcon {
    /// The warning triangle, drawn for an [`AlertDialog`].
    Warning,
    /// The question mark, drawn for a [`ConfirmDialog`].
    Question,
    /// The information mark, drawn for an [`AboutDialog`].
    Info,
}

impl CardIcon {
    /// The outline glyph drawn for this severity, from the shared icon registry.
    fn name(self) -> IconName {
        match self {
            CardIcon::Warning => IconName::Warning,
            CardIcon::Question => IconName::Help,
            CardIcon::Info => IconName::Info,
        }
    }

    /// The severity glyph for a dialog role: an alert warns, a confirmation asks, an about card
    /// informs. The role is the dialog's identity here — the same key the palette resolves by —
    /// so the mapping lives beside it instead of travelling through one more parameter.
    fn for_role(role: &str) -> Option<Self> {
        match role {
            "alert_dialog" => Some(CardIcon::Warning),
            "confirm_dialog" => Some(CardIcon::Question),
            "about_dialog" => Some(CardIcon::Info),
            _ => None,
        }
    }

    /// The colour for this severity: the theme's semantic token, so the glyph follows the active
    /// appearance instead of staying light-blue on a dark dialog.
    fn color(self) -> Color {
        let token = match self {
            CardIcon::Warning => SemanticColor::Warning,
            CardIcon::Question | CardIcon::Info => SemanticColor::Info,
        };
        // `semantic_color` takes and releases the manager's lock and returns an owned colour, so
        // no guard outlives the call.
        crate::style::semantic_color(token).unwrap_or_else(|| self.color_fallback())
    }

    /// The literal last resort behind [`Self::color`], used only when no theme is active and
    /// therefore no token can be resolved.
    fn color_fallback(self) -> Color {
        match self {
            CardIcon::Warning => Color::rgb(255, 140, 0),
            CardIcon::Question | CardIcon::Info => Color::rgb(0, 120, 215),
        }
    }
}

/// The width one glyph advance occupies, which is the renderer's measurement contract.
fn advance(font: &Font) -> u32 {
    font.size().ceil().max(1.0) as u32
}

/// The action row's button rectangles, laid out right to left in `labels` order.
///
/// Returned in `labels` order (the first label is the leftmost button), so a caller indexing
/// the row by its own button index cannot be off by a mirror. Each button's width comes from
/// the renderer's own measurement, so a CJK or other wide label is sized by the ink it will
/// paint instead of by a character count that treats every glyph as one Latin advance.
fn button_rects(context: &RenderContext, rect: Rect, labels: &[&str], font: &Font) -> Vec<Rect> {
    if labels.is_empty() || rect.width == 0 || rect.height == 0 {
        return Vec::new();
    }
    let height = BUTTON_HEIGHT.min(rect.height);
    let y = rect
        .y
        .saturating_add(rect.height as i32)
        .saturating_sub(CARD_PADDING as i32)
        .saturating_sub(height as i32);
    let mut right = rect.x.saturating_add(rect.width as i32).saturating_sub(CARD_PADDING as i32);
    let mut rects = vec![Rect::new(0, 0, 0, 0); labels.len()];
    for (index, label) in labels.iter().enumerate().rev() {
        let width = (context.measure_text(label, font).width)
            .saturating_add(2 * BUTTON_PADDING)
            .min(rect.width);
        let x = right.saturating_sub(width as i32);
        rects[index] = Rect::new(x, y, width, height);
        right = x.saturating_sub(BUTTON_GAP as i32);
    }
    rects
}

/// Breaks `text` into lines of at most `columns` characters, on word boundaries.
///
/// A word longer than the line is hard-split rather than dropped, so a long URL in a message is
/// still readable instead of vanishing. The renderer's advance is one glyph per cluster, so a
/// character budget and a pixel budget are the same number.
///
/// # Why this is not `text_utils::wrap_to_width`
///
/// The rule here is **fill the line, then continue**: the part of a long word that fits in the
/// room left beside the previous words stays on that line, and only the remainder moves down.
/// The shared splitter moves such a word down whole and splits it only when it is alone on a
/// line, so adopting it would re-flow every dialog body (and change the dialog snapshots). Until
/// `wrap_to_width` takes a policy for that choice, this loop is the deliberate local answer.
pub(crate) fn wrap_words(text: &str, columns: usize) -> Vec<String> {
    if columns == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for word in text.split_whitespace() {
        let mut remaining = word;
        loop {
            let space = usize::from(!current.is_empty());
            let room = columns.saturating_sub(current_len + space);
            if room == 0 {
                lines.push(std::mem::take(&mut current));
                current_len = 0;
                continue;
            }
            let take = remaining.chars().count().min(room);
            if take == 0 {
                break;
            }
            if space == 1 {
                current.push(' ');
                current_len += 1;
            }
            let split: String = remaining.chars().take(take).collect();
            current.push_str(&split);
            current_len += take;
            remaining = &remaining[split.len()..];
            if remaining.is_empty() {
                break;
            }
            // The word continues on the next line.
            lines.push(std::mem::take(&mut current));
            current_len = 0;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// The line-wrapping results one card has already derived.
///
/// A card paints three text blocks on every frame, and each block used to re-run the word
/// splitter and allocate a fresh line list. The result is a pure function of (text, columns),
/// so it is cached per card and reused until the text changes (G-P15).
#[derive(Default)]
struct WrapCache {
    entries: RefCell<HashMap<(u64, usize), Rc<Vec<String>>>>,
}

impl WrapCache {
    /// The wrapped lines of `text` at `columns`, from the cache when possible.
    fn lines(&self, text: &str, columns: usize) -> Rc<Vec<String>> {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let key = (hasher.finish(), columns);
        if let Some(lines) = self.entries.borrow().get(&key) {
            return Rc::clone(lines);
        }
        let lines = Rc::new(wrap_words(text, columns));
        let mut entries = self.entries.borrow_mut();
        // A card owns a handful of blocks; a replacing message should not grow the map without
        // bound, so the stale entries are dropped when a new text arrives.
        if entries.len() >= 16 {
            entries.clear();
        }
        entries.insert(key, Rc::clone(&lines));
        lines
    }
}

/// Everything a card paints, grouped so adding a row cannot silently leave one dialog behind.
struct CardContent<'a> {
    role: &'a str,
    title: &'a str,
    message: &'a str,
    /// A second, smaller line below the message (`informativeText`).
    informative: &'a str,
    /// A further, muted block below the informative line (`detailedText`).
    detailed: &'a str,
    /// Extra label/value rows (`AboutDialog`'s application and version).
    extra_lines: &'a [String],
    buttons: &'a [&'a str],
    hovered: Option<usize>,
    /// The action button the keyboard focus ring is on.
    focused: Option<usize>,
    /// A caller-supplied severity glyph; `None` keeps the role's own glyph.
    icon: Option<IconName>,
    /// An optional check box row above the buttons: its label and its state.
    check_box: Option<(&'a str, bool)>,
    /// Whether the card dims the page behind it.
    modal: bool,
    /// How far the card has revealed, `0.0` hidden and `1.0` settled.
    reveal: f32,
    /// The wrapped-line cache shared by the card's text blocks.
    wrap_cache: &'a WrapCache,
    /// The action Enter activates, marked with an emphasis ring (DLG-04).
    default_button: Option<usize>,
}

/// The typography and stopping rule for one block of wrapped text.
struct BlockStyle<'a> {
    font: &'a Font,
    ink: Color,
    columns: usize,
    line_height: i32,
    stop_y: i32,
}

/// Paints one block of wrapped text and advances `y` past it.
fn paint_block(
    ctx: &mut RenderContext,
    rect: Rect,
    y: &mut i32,
    text: &str,
    style: &BlockStyle<'_>,
    cache: &WrapCache,
) {
    if text.is_empty() {
        return;
    }
    let content_width = rect.width.saturating_sub(2 * CARD_PADDING);
    for line in cache.lines(text, style.columns).iter() {
        if *y + style.line_height > style.stop_y {
            break;
        }
        let band = Rect::new(
            rect.x.saturating_add(CARD_PADDING as i32),
            *y,
            content_width,
            style.line_height as u32,
        );
        ctx.draw_text_line_cow(band, line, style.font, style.ink, HorizontalAlignment::Left);
        *y += style.line_height;
    }
}

/// Paints the shared card and returns the action row, the check box row (when present) and
/// the frame it painted into.
fn paint_card(
    ctx: &mut RenderContext,
    base: &BaseWidget,
    card: &CardContent<'_>,
) -> (Rect, Vec<Rect>, Option<Rect>) {
    let geometry = base.geometry();
    let reveal = card.reveal.clamp(0.0, 1.0);
    if reveal <= 0.0 {
        return (Rect::new(0, 0, 0, 0), Vec::new(), None);
    }
    if card.modal {
        super::draw_modal_scrim_scaled(ctx, base.id(), geometry, base.parent(), reveal);
    }
    let rect = super::scale_about_centre(
        geometry,
        super::REVEAL_MIN_SCALE + (1.0 - super::REVEAL_MIN_SCALE) * reveal,
    );
    if rect.width == 0 || rect.height == 0 {
        return (rect, Vec::new(), None);
    }
    let palette = card_palette(base, card.role);
    // The card is a raised surface: a fill a hair off the window's own, over a border, which is
    // what makes it read as a panel in both appearances.
    ctx.fill_rounded_rect(rect, CARD_RADIUS, palette.surface);
    ctx.draw_rounded_rect_stroke(rect, CARD_RADIUS, palette.border, 1);

    let glyph = advance(&palette.font);
    let content_width = rect.width.saturating_sub(2 * CARD_PADDING);

    // The title, then the message below it. Both are bounded to the content box. The severity
    // icon, when present, shares the title's row: the text starts after it, so a title never
    // collides with its own glyph, and a card without a title still reserves the icon's line so
    // the message cannot start underneath it.
    let mut y = rect.y.saturating_add(CARD_PADDING as i32);
    let mut text_left = rect.x.saturating_add(CARD_PADDING as i32);
    let mut text_width = content_width;
    let icon = card.icon.or_else(|| CardIcon::for_role(card.role).map(CardIcon::name));
    if let Some(icon) = icon {
        // One glyph square, sized from the title's own metrics and clamped to the card, so a
        // squeezed card shrinks the glyph instead of letting it escape the frame.
        let side = advance(&palette.title_font).max(12).min(rect.width).min(rect.height);
        let icon_rect = Rect::new(text_left, y, side, side);
        // A caller-supplied glyph is painted in the card's ink; the role's own glyph keeps the
        // semantic severity colour, which is the part that carries meaning.
        let color = if card.icon.is_some() {
            palette.ink
        } else {
            CardIcon::for_role(card.role).map(CardIcon::color).unwrap_or(palette.ink)
        };
        crate::widget::draw_icon_at(ctx, icon_rect, color, icon);
        let gutter = side as i32 + ICON_TITLE_GAP as i32;
        text_left = text_left.saturating_add(gutter);
        text_width = text_width.saturating_sub(gutter as u32);
        if card.title.is_empty() {
            y = y.saturating_add(side as i32).saturating_add(TITLE_GAP as i32);
        }
    }
    if !card.title.is_empty() {
        let band = Rect::new(text_left, y, text_width, advance(&palette.title_font) + 4);
        ctx.draw_text_line_cow(
            band,
            card.title,
            &palette.title_font,
            palette.ink,
            HorizontalAlignment::Left,
        );
        y = y.saturating_add(band.height as i32).saturating_add(TITLE_GAP as i32);
    }

    let max_columns = (content_width / glyph.max(1)).max(1) as usize;
    let line_height = glyph as i32 + 2;
    let body_ink = palette.ink.with_alpha_f32(0.85);
    let stop_y = rect
        .y
        .saturating_add(rect.height as i32)
        .saturating_sub(CARD_PADDING as i32)
        .saturating_sub(BUTTON_HEIGHT as i32)
        .saturating_sub(BUTTON_GAP as i32);

    // Extra label/value lines (the `AboutDialog`'s application and version rows) come before
    // the message, because they identify the subject rather than describe it.
    for line in card.extra_lines {
        let band = Rect::new(
            rect.x.saturating_add(CARD_PADDING as i32),
            y,
            content_width,
            line_height as u32,
        );
        ctx.draw_text_line_cow(band, line, &palette.font, body_ink, HorizontalAlignment::Left);
        y = y.saturating_add(line_height);
    }

    paint_block(
        ctx,
        rect,
        &mut y,
        card.message,
        &BlockStyle {
            font: &palette.font,
            ink: body_ink,
            columns: max_columns,
            line_height,
            stop_y,
        },
        card.wrap_cache,
    );
    // The informative line is a step stronger than the body: it qualifies the message rather
    // than repeating it, and the distinction is what a two-level alert is for.
    paint_block(
        ctx,
        rect,
        &mut y,
        card.informative,
        &BlockStyle {
            font: &palette.font,
            ink: palette.ink,
            columns: max_columns,
            line_height,
            stop_y,
        },
        card.wrap_cache,
    );
    paint_block(
        ctx,
        rect,
        &mut y,
        card.detailed,
        &BlockStyle {
            font: &palette.font,
            ink: palette.ink.with_alpha_f32(0.7),
            columns: max_columns,
            line_height,
            stop_y,
        },
        card.wrap_cache,
    );

    let rects = button_rects(ctx, rect, card.buttons, &palette.font);
    let check_box_rect = card.check_box.map(|(label, checked)| {
        let row_h = BUTTON_HEIGHT.min(rect.height);
        let row_y = rects
            .first()
            .map(|first| first.y.saturating_sub(row_h as i32).saturating_sub(4))
            .unwrap_or_else(|| {
                rect.y
                    .saturating_add(rect.height as i32)
                    .saturating_sub(CARD_PADDING as i32)
                    .saturating_sub(row_h as i32)
            });
        let row = Rect::new(
            rect.x.saturating_add(CARD_PADDING as i32),
            row_y,
            rect.width.saturating_sub(2 * CARD_PADDING),
            row_h,
        );
        let box_side = (row_h.saturating_sub(8)).min(14);
        let box_rect =
            Rect::new(row.x, row.y + ((row_h - box_side) / 2) as i32, box_side, box_side);
        ctx.fill_rect(box_rect, palette.surface);
        ctx.draw_rect(box_rect, palette.border);
        if checked {
            let inset = 3.min(box_side as i32 / 2);
            ctx.fill_rect(
                Rect::new(
                    box_rect.x + inset,
                    box_rect.y + inset,
                    box_side.saturating_sub((inset * 2) as u32),
                    box_side.saturating_sub((inset * 2) as u32),
                ),
                palette.accent,
            );
        }
        let label_band = Rect::new(
            box_rect.x + box_side as i32 + BUTTON_GAP as i32,
            row.y,
            row.width.saturating_sub(box_side + BUTTON_GAP),
            row_h,
        );
        ctx.draw_text_line_cow(
            label_band,
            label,
            &palette.font,
            palette.ink,
            HorizontalAlignment::Left,
        );
        row
    });

    for (index, rect) in rects.iter().enumerate() {
        let (fill, text_ink, border) = if card.hovered == Some(index) {
            (palette.accent, palette.accent.contrast_color(), palette.accent)
        } else {
            (
                palette.accent.with_alpha_f32(0.12),
                palette.accent,
                palette.accent.with_alpha_f32(0.6),
            )
        };
        ctx.fill_rounded_rect(*rect, 4, fill);
        ctx.draw_rounded_rect_stroke(*rect, 4, border, 1);
        ctx.draw_text_line_cow(
            *rect,
            card.buttons[index],
            &palette.font,
            text_ink,
            HorizontalAlignment::Center,
        );
        if card.focused == Some(index) {
            ctx.draw_rounded_rect_stroke(*rect, 4, palette.accent, 2);
        } else if card.default_button == Some(index) {
            // The default action carries the accent even when the keyboard focus ring is
            // elsewhere, which is what makes "Enter will do this" visible before it is used
            // (DLG-04). A hovered button already uses the accent fill, so the ring is skipped.
            ctx.draw_rounded_rect_stroke(*rect, 4, palette.accent, 2);
        }
    }
    (rect, rects, check_box_rect)
}

/// The index of the button under `point`, if any.
fn button_at(rects: &[Rect], point: crate::core::Point) -> Option<usize> {
    rects.iter().position(|rect| rect.contains(point))
}

/// The state every notice card shares: severity glyph override, the two extra text levels,
/// the default action, the optional check box, its own preferred size and its reveal.
///
/// Grouped rather than copied into each of the three dialogs because these are the fields a
/// fourth notice would otherwise forget: the three cards draw from one painter, so their state
/// should travel as one value.
struct CardState {
    /// Whether the card dims the page behind it.
    modal: bool,
    /// A caller-supplied severity glyph; `None` keeps the role's own.
    icon: Option<IconName>,
    /// The `informativeText` line below the message.
    informative_text: String,
    /// The `detailedText` block below the informative line.
    detailed_text: String,
    /// The index of the action Enter activates.
    default_button: usize,
    /// The check box label; empty means no check box is painted.
    check_box_text: String,
    /// The check box state.
    check_box_checked: bool,
    /// The size the control asks a layout for.
    preferred_size: Size,
    /// How far the card has revealed.
    reveal: PropertyDriver,
    /// The base visibility sampled on the last tick.
    new_visible: bool,
    /// Wrapped-line results, reused while the card's text is unchanged (G-P15).
    wrap_cache: WrapCache,
}

impl CardState {
    /// A settled, visible, modal card at `preferred_size`.
    fn new(preferred_size: Size) -> Self {
        Self {
            modal: true,
            icon: None,
            informative_text: String::new(),
            detailed_text: String::new(),
            default_button: 0,
            check_box_text: String::new(),
            check_box_checked: false,
            preferred_size,
            // A freshly built dialog is visible and settled: the reveal is what a later
            // show/hide moves, not what the first frame fades in from.
            reveal: PropertyDriver::at(1.0, MotionSlot::Normal),
            new_visible: true,
            wrap_cache: WrapCache::default(),
        }
    }

    /// Follows the base visibility and advances the reveal.
    fn tick(&mut self, visible: bool, delta_ms: u32) -> bool {
        if visible != self.new_visible {
            self.new_visible = visible;
            self.reveal.set_target(if visible { 1.0 } else { 0.0 });
        }
        self.reveal.tick(delta_ms)
    }

    /// Aims the reveal at the state a show/hide just selected.
    fn aim(&mut self, visible: bool) {
        self.new_visible = visible;
        self.reveal.set_target(if visible { 1.0 } else { 0.0 });
    }

    /// Whether the reveal is between two states.
    fn is_animating(&self) -> bool {
        self.reveal.is_moving()
    }

    /// The drawn reveal fraction.
    fn reveal(&self) -> f32 {
        self.reveal.value()
    }
}

/// Builds the buttons for a card on the press path, where there is no render context.
fn measured_buttons(geometry: Rect, role: &str, base: &BaseWidget, labels: &[&str]) -> Vec<Rect> {
    let size = Size::new(geometry.width.max(1), geometry.height.max(1));
    let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
    let context = RenderContext::new(&mut backend);
    let font = card_palette(base, role).font;
    button_rects(&context, geometry, labels, &font)
}

// ── AlertDialog ─────────────────────────────────────────────────────────────

/// AlertDialog — a modal notice with one acknowledging action.
pub struct AlertDialog {
    base: BaseWidget,
    title: String,
    message: String,
    button_text: String,
    hovered: Option<usize>,
    /// Shared notice-card state: modality, glyph, extra text, default action, check box, reveal.
    card: CardState,
    /// Emitted when the acknowledgement button is pressed.
    pub accepted: GenericSignal,
    /// Emitted when the dialog is dismissed without acknowledging (Escape or a close request).
    pub rejected: GenericSignal,
    /// Emitted when Tab moves the focus ring; the payload is this dialog's own object id.
    pub focus_changed: Signal1<ObjectId>,
    /// The action button the keyboard focus ring is on.
    focus: Option<usize>,
}

impl AlertDialog {
    /// Creates an alert titled `title` with `message` and an "OK" action.
    pub fn new(title: &str, message: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AlertDialog, geometry, "AlertDialog"),
            title: title.to_string(),
            message: message.to_string(),
            button_text: "OK".to_string(),
            hovered: None,
            card: CardState::new(Size::new(320, 160)),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            focus_changed: Signal1::new(),
            focus: None,
        }
    }

    /// The dialog's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the dialog's title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// The dialog's message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces the dialog's message.
    pub fn set_message(&mut self, message: String) {
        if self.message == message {
            return;
        }
        self.message = message;
        self.base.request_redraw();
    }

    /// The acknowledgement button's label.
    pub fn button_text(&self) -> &str {
        &self.button_text
    }

    /// Replaces the acknowledgement button's label.
    pub fn set_button_text(&mut self, button_text: String) {
        if self.button_text == button_text {
            return;
        }
        self.button_text = button_text;
        self.base.request_redraw();
    }

    /// The action row's rects.
    fn buttons(&self) -> Vec<Rect> {
        measured_buttons(self.geometry(), "alert_dialog", &self.base, &[self.button_text.as_str()])
    }

    /// Whether the card dims the page behind it. Defaults to `true`.
    pub fn is_modal(&self) -> bool {
        self.card.modal
    }

    /// Sets the modality intent and repaints when it changes.
    pub fn set_modal(&mut self, modal: bool) {
        if self.card.modal == modal {
            return;
        }
        self.card.modal = modal;
        self.base.request_redraw();
    }

    /// The caller-supplied severity glyph, or `None` for the role's own.
    pub fn icon(&self) -> Option<IconName> {
        self.card.icon
    }

    /// Replaces the severity glyph and repaints. `None` restores the alert glyph.
    pub fn set_icon(&mut self, icon: Option<IconName>) {
        if self.card.icon == icon {
            return;
        }
        self.card.icon = icon;
        self.base.request_redraw();
    }

    /// The informative text painted below the message.
    pub fn informative_text(&self) -> &str {
        &self.card.informative_text
    }

    /// Replaces the informative text and repaints.
    pub fn set_informative_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.informative_text == text {
            return;
        }
        self.card.informative_text = text;
        self.base.request_redraw();
    }

    /// The detailed text painted below the informative line.
    pub fn detailed_text(&self) -> &str {
        &self.card.detailed_text
    }

    /// Replaces the detailed text and repaints.
    pub fn set_detailed_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.detailed_text == text {
            return;
        }
        self.card.detailed_text = text;
        self.base.request_redraw();
    }

    /// The action Enter activates, as an index into the card's button row.
    pub fn default_button(&self) -> usize {
        self.card.default_button
    }

    /// Sets the action Enter activates and repaints when the highlighted button changes.
    pub fn set_default_button(&mut self, index: usize) {
        if self.card.default_button == index {
            return;
        }
        self.card.default_button = index;
        self.base.request_redraw();
    }

    /// The optional check box row's label, or `""` when no row is painted.
    pub fn check_box_text(&self) -> &str {
        &self.card.check_box_text
    }

    /// Installs a check box row above the actions. An empty label removes it.
    pub fn set_check_box(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.card.check_box_text == label {
            return;
        }
        self.card.check_box_text = label;
        self.base.request_redraw();
    }

    /// Whether the check box is checked.
    pub fn is_check_box_checked(&self) -> bool {
        self.card.check_box_checked
    }

    /// Sets the check box state and repaints when it changes.
    pub fn set_check_box_checked(&mut self, checked: bool) {
        if self.card.check_box_checked == checked {
            return;
        }
        self.card.check_box_checked = checked;
        self.base.request_redraw();
    }

    /// How far the card has revealed, `0.0` hidden and `1.0` settled.
    pub fn reveal_progress(&self) -> f32 {
        self.card.reveal()
    }

    /// Advances the reveal and reports whether another frame is needed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    /// Whether the reveal is moving.
    pub fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    /// The check box row's rect, if a row is installed and the card has been painted.
    fn check_box_rect(&self) -> Option<Rect> {
        if self.card.check_box_text.is_empty() {
            return None;
        }
        let row_h = BUTTON_HEIGHT.min(self.geometry().height);
        let buttons = self.buttons();
        let row_y = buttons
            .first()
            .map(|first| first.y.saturating_sub(row_h as i32).saturating_sub(4))
            .unwrap_or_else(|| {
                self.geometry().y + self.geometry().height as i32
                    - CARD_PADDING as i32
                    - row_h as i32
            });
        Some(Rect::new(
            self.geometry().x + CARD_PADDING as i32,
            row_y,
            self.geometry().width.saturating_sub(2 * CARD_PADDING),
            row_h,
        ))
    }

    /// Routes an activated action index: the alert has one acknowledgement.
    fn activate(&mut self, _index: usize) {
        self.accepted.emit();
    }
}

impl Widget for AlertDialog {
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
        self.card.preferred_size
    }

    fn show(&mut self) {
        self.base.show();
        self.card.aim(true);
        self.base.request_redraw();
    }

    fn hide(&mut self) {
        self.base.hide();
        self.card.aim(false);
        self.base.request_redraw();
    }

    fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            "focus_changed" => {
                Some(EventSignalRef::mapped("focus_changed", &self.focus_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for AlertDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "message" => Ok(CapabilityValue::String(self.message.clone())),
            "button_text" => Ok(CapabilityValue::String(self.button_text.clone())),
            "informative_text" => Ok(CapabilityValue::String(self.informative_text().to_string())),
            "detailed_text" => Ok(CapabilityValue::String(self.detailed_text().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "default_button" => Ok(CapabilityValue::UInt(self.default_button() as u64)),
            "check_box" => Ok(CapabilityValue::String(self.check_box_text().to_string())),
            "check_box_checked" => Ok(CapabilityValue::Bool(self.is_check_box_checked())),
            "icon" => Ok(CapabilityValue::String(
                self.icon().map(|icon| icon.as_str().to_string()).unwrap_or_default(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "message" => {
                self.set_message(expect_string(value)?);
                Ok(())
            }
            "button_text" => {
                self.set_button_text(expect_string(value)?);
                Ok(())
            }
            "informative_text" => {
                self.set_informative_text(expect_string(value)?);
                Ok(())
            }
            "detailed_text" => {
                self.set_detailed_text(expect_string(value)?);
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "default_button" => {
                let index = match value {
                    CapabilityValue::UInt(v) => v as usize,
                    CapabilityValue::Int(v) if v >= 0 => v as usize,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_default_button(index);
                Ok(())
            }
            "check_box" => {
                self.set_check_box(expect_string(value)?);
                Ok(())
            }
            "check_box_checked" => {
                self.set_check_box_checked(expect_bool(value)?);
                Ok(())
            }
            "icon" => {
                let token = expect_string(value)?;
                let icon = if token.is_empty() {
                    None
                } else {
                    Some(IconName::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?)
                };
                self.set_icon(icon);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "message",
            "button_text",
            "informative_text",
            "detailed_text",
            "modal",
            "default_button",
            "check_box",
            "check_box_checked",
            "icon",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for AlertDialog {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let content = CardContent {
            role: "alert_dialog",
            title: &self.title,
            message: &self.message,
            informative: &self.card.informative_text,
            detailed: &self.card.detailed_text,
            extra_lines: &[],
            buttons: &[self.button_text.as_str()],
            hovered: self.hovered,
            focused: self.focus,
            icon: self.card.icon,
            check_box: (!self.card.check_box_text.is_empty())
                .then_some((self.card.check_box_text.as_str(), self.card.check_box_checked)),
            modal: self.card.modal,
            reveal: self.card.reveal(),
            wrap_cache: &self.card.wrap_cache,
            default_button: Some(self.card.default_button),
        };
        let _ = paint_card(ctx, &self.base, &content);
    }
}

impl EventHandler for AlertDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let hovered = button_at(&self.buttons(), *pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = button_at(&self.buttons(), *pos) {
                    self.focus = Some(index);
                    self.activate(index);
                    self.base.request_redraw();
                    return;
                }
                if self.check_box_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.set_check_box_checked(!self.card.check_box_checked);
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == key_codes::TAB {
                    let count = self.buttons().len();
                    let next = super::move_focus(self.focus, count, true);
                    if next != self.focus {
                        self.focus = next;
                        self.focus_changed.emit(self.id());
                        self.base.request_redraw();
                    }
                } else if *key == key_codes::ENTER || *key == key_codes::LINE_FEED {
                    let button = self.focus.filter(|index| *index < self.buttons().len());
                    self.activate(button.unwrap_or(self.card.default_button));
                } else if *key == key_codes::ESCAPE {
                    self.rejected.emit();
                }
            }
            _ => {}
        }
    }
}

// ── ConfirmDialog ──────────────────────────────────────────────────────────────────────────

/// ConfirmDialog — a modal question with an accept and a reject action.
///
/// `accept_text` is the confirming action and `reject_text` the dismissing one. Order in the
/// action row follows the platform convention this crate's other dialogs use: the confirming
/// action on the right, so `Enter` and the rightmost button agree.
pub struct ConfirmDialog {
    base: BaseWidget,
    title: String,
    message: String,
    accept_text: String,
    reject_text: String,
    hovered: Option<usize>,
    /// Shared notice-card state.
    card: CardState,
    /// Emitted when the confirming action is pressed.
    pub accepted: GenericSignal,
    /// Emitted when the dismissing action is pressed.
    pub rejected: GenericSignal,
    /// Emitted when Tab moves the focus ring; the payload is this dialog's own object id.
    pub focus_changed: Signal1<ObjectId>,
    /// The action button the keyboard focus ring is on.
    focus: Option<usize>,
}

impl ConfirmDialog {
    /// Creates a question titled `title` with "Cancel" / "OK" actions.
    pub fn new(title: &str, message: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ConfirmDialog, geometry, "ConfirmDialog"),
            title: title.to_string(),
            message: message.to_string(),
            accept_text: "OK".to_string(),
            reject_text: "Cancel".to_string(),
            hovered: None,
            card: CardState::new(Size::new(360, 170)),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            focus_changed: Signal1::new(),
            focus: None,
        }
    }

    /// Whether the card dims the page behind it. Defaults to `true`.
    pub fn is_modal(&self) -> bool {
        self.card.modal
    }

    /// Sets the modality intent and repaints when it changes.
    pub fn set_modal(&mut self, modal: bool) {
        if self.card.modal == modal {
            return;
        }
        self.card.modal = modal;
        self.base.request_redraw();
    }

    /// The caller-supplied severity glyph, or `None` for the confirmation glyph.
    pub fn icon(&self) -> Option<IconName> {
        self.card.icon
    }

    /// Replaces the severity glyph and repaints. `None` restores the question glyph.
    pub fn set_icon(&mut self, icon: Option<IconName>) {
        if self.card.icon == icon {
            return;
        }
        self.card.icon = icon;
        self.base.request_redraw();
    }

    /// The informative text painted below the message.
    pub fn informative_text(&self) -> &str {
        &self.card.informative_text
    }

    /// Replaces the informative text and repaints.
    pub fn set_informative_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.informative_text == text {
            return;
        }
        self.card.informative_text = text;
        self.base.request_redraw();
    }

    /// The detailed text painted below the informative line.
    pub fn detailed_text(&self) -> &str {
        &self.card.detailed_text
    }

    /// Replaces the detailed text and repaints.
    pub fn set_detailed_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.detailed_text == text {
            return;
        }
        self.card.detailed_text = text;
        self.base.request_redraw();
    }

    /// The action Enter activates, as an index into the row (0 rejects, 1 accepts).
    pub fn default_button(&self) -> usize {
        self.card.default_button
    }

    /// Sets the action Enter activates and repaints when it changes.
    pub fn set_default_button(&mut self, index: usize) {
        if self.card.default_button == index {
            return;
        }
        self.card.default_button = index;
        self.base.request_redraw();
    }

    /// The optional check box row's label, or `""` when none is painted.
    pub fn check_box_text(&self) -> &str {
        &self.card.check_box_text
    }

    /// Installs a check box row above the actions. An empty label removes it.
    pub fn set_check_box(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.card.check_box_text == label {
            return;
        }
        self.card.check_box_text = label;
        self.base.request_redraw();
    }

    /// Whether the check box is checked.
    pub fn is_check_box_checked(&self) -> bool {
        self.card.check_box_checked
    }

    /// Sets the check box state and repaints when it changes.
    pub fn set_check_box_checked(&mut self, checked: bool) {
        if self.card.check_box_checked == checked {
            return;
        }
        self.card.check_box_checked = checked;
        self.base.request_redraw();
    }

    /// How far the card has revealed.
    pub fn reveal_progress(&self) -> f32 {
        self.card.reveal()
    }

    /// Advances the reveal and reports whether another frame is needed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    /// Whether the reveal is moving.
    pub fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    /// The dialog's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the dialog's title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// The dialog's message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces the dialog's message.
    pub fn set_message(&mut self, message: String) {
        if self.message == message {
            return;
        }
        self.message = message;
        self.base.request_redraw();
    }

    /// The confirming action's label.
    pub fn accept_text(&self) -> &str {
        &self.accept_text
    }

    /// Replaces the confirming action's label.
    pub fn set_accept_text(&mut self, text: String) {
        if self.accept_text == text {
            return;
        }
        self.accept_text = text;
        self.base.request_redraw();
    }

    /// The dismissing action's label.
    pub fn reject_text(&self) -> &str {
        &self.reject_text
    }

    /// Replaces the dismissing action's label.
    pub fn set_reject_text(&mut self, text: String) {
        if self.reject_text == text {
            return;
        }
        self.reject_text = text;
        self.base.request_redraw();
    }

    /// The action row's labels, left to right: reject, then accept.
    fn labels(&self) -> [&str; 2] {
        [self.reject_text.as_str(), self.accept_text.as_str()]
    }

    /// The action row's rects, in [`Self::labels`] order.
    fn buttons(&self) -> Vec<Rect> {
        let labels = self.labels();
        measured_buttons(self.geometry(), "confirm_dialog", &self.base, &labels)
    }

    /// The check box row's rect, when a row is installed.
    fn check_box_rect(&self) -> Option<Rect> {
        if self.card.check_box_text.is_empty() {
            return None;
        }
        let row_h = BUTTON_HEIGHT.min(self.geometry().height);
        let row_y = self
            .buttons()
            .first()
            .map(|first| first.y.saturating_sub(row_h as i32).saturating_sub(4))
            .unwrap_or_else(|| {
                self.geometry().y + self.geometry().height as i32
                    - CARD_PADDING as i32
                    - row_h as i32
            });
        Some(Rect::new(
            self.geometry().x + CARD_PADDING as i32,
            row_y,
            self.geometry().width.saturating_sub(2 * CARD_PADDING),
            row_h,
        ))
    }

    /// Routes an activated action index (0 rejects, everything else accepts).
    fn activate(&mut self, index: usize) {
        if index == 0 {
            self.rejected.emit();
        } else {
            self.accepted.emit();
        }
    }
}

impl Widget for ConfirmDialog {
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
        self.card.preferred_size
    }

    fn show(&mut self) {
        self.base.show();
        self.card.aim(true);
        self.base.request_redraw();
    }

    fn hide(&mut self) {
        self.base.hide();
        self.card.aim(false);
        self.base.request_redraw();
    }

    fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            "focus_changed" => {
                Some(EventSignalRef::mapped("focus_changed", &self.focus_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for ConfirmDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "message" => Ok(CapabilityValue::String(self.message.clone())),
            "accept_text" => Ok(CapabilityValue::String(self.accept_text.clone())),
            "reject_text" => Ok(CapabilityValue::String(self.reject_text.clone())),
            "informative_text" => Ok(CapabilityValue::String(self.informative_text().to_string())),
            "detailed_text" => Ok(CapabilityValue::String(self.detailed_text().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "default_button" => Ok(CapabilityValue::UInt(self.default_button() as u64)),
            "check_box" => Ok(CapabilityValue::String(self.check_box_text().to_string())),
            "check_box_checked" => Ok(CapabilityValue::Bool(self.is_check_box_checked())),
            "icon" => Ok(CapabilityValue::String(
                self.icon().map(|icon| icon.as_str().to_string()).unwrap_or_default(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "message" => {
                self.set_message(expect_string(value)?);
                Ok(())
            }
            "accept_text" => {
                self.set_accept_text(expect_string(value)?);
                Ok(())
            }
            "reject_text" => {
                self.set_reject_text(expect_string(value)?);
                Ok(())
            }
            "informative_text" => {
                self.set_informative_text(expect_string(value)?);
                Ok(())
            }
            "detailed_text" => {
                self.set_detailed_text(expect_string(value)?);
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "default_button" => {
                let index = match value {
                    CapabilityValue::UInt(v) => v as usize,
                    CapabilityValue::Int(v) if v >= 0 => v as usize,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_default_button(index);
                Ok(())
            }
            "check_box" => {
                self.set_check_box(expect_string(value)?);
                Ok(())
            }
            "check_box_checked" => {
                self.set_check_box_checked(expect_bool(value)?);
                Ok(())
            }
            "icon" => {
                let token = expect_string(value)?;
                let icon = if token.is_empty() {
                    None
                } else {
                    Some(IconName::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?)
                };
                self.set_icon(icon);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "message",
            "accept_text",
            "reject_text",
            "informative_text",
            "detailed_text",
            "modal",
            "default_button",
            "check_box",
            "check_box_checked",
            "icon",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for ConfirmDialog {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let labels = self.labels();
        let content = CardContent {
            role: "confirm_dialog",
            title: &self.title,
            message: &self.message,
            informative: &self.card.informative_text,
            detailed: &self.card.detailed_text,
            extra_lines: &[],
            buttons: &labels,
            hovered: self.hovered,
            focused: self.focus,
            icon: self.card.icon,
            check_box: (!self.card.check_box_text.is_empty())
                .then_some((self.card.check_box_text.as_str(), self.card.check_box_checked)),
            modal: self.card.modal,
            reveal: self.card.reveal(),
            wrap_cache: &self.card.wrap_cache,
            default_button: Some(self.card.default_button),
        };
        let _ = paint_card(ctx, &self.base, &content);
    }
}

impl EventHandler for ConfirmDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let hovered = button_at(&self.buttons(), *pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = button_at(&self.buttons(), *pos) {
                    self.focus = Some(index);
                    self.activate(index);
                    return;
                }
                if self.check_box_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.set_check_box_checked(!self.card.check_box_checked);
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == key_codes::TAB {
                    let count = self.buttons().len();
                    let next = super::move_focus(self.focus, count, true);
                    if next != self.focus {
                        self.focus = next;
                        self.focus_changed.emit(self.id());
                        self.base.request_redraw();
                    }
                } else if *key == key_codes::ENTER || *key == key_codes::LINE_FEED {
                    let button = self.focus.filter(|index| *index < self.buttons().len());
                    self.activate(button.unwrap_or(self.card.default_button));
                } else if *key == key_codes::ESCAPE {
                    self.rejected.emit();
                }
            }
            _ => {}
        }
    }
}

// ── AboutDialog ─────────────────────────────────────────────────────────────

/// AboutDialog — the standard application-information dialog.
///
/// Shows the application name, its version and a short message, and one dismissing action.
/// The three facts are separate properties rather than one pre-formatted line, so a designer
/// can bind the version to a release source without rebuilding the text.
pub struct AboutDialog {
    base: BaseWidget,
    title: String,
    application_name: String,
    version: String,
    message: String,
    hovered: Option<usize>,
    /// Shared notice-card state.
    card: CardState,
    /// Emitted when the dismissal action is pressed.
    pub closed: GenericSignal,
}

impl AboutDialog {
    /// Creates an about dialog whose dismissal action reads "Close".
    pub fn new(application_name: &str, version: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AboutDialog, geometry, "AboutDialog"),
            title: "About".to_string(),
            application_name: application_name.to_string(),
            version: version.to_string(),
            message: String::new(),
            hovered: None,
            card: CardState::new(Size::new(360, 200)),
            closed: GenericSignal::new(),
        }
    }

    /// Whether the card dims the page behind it. Defaults to `true`.
    pub fn is_modal(&self) -> bool {
        self.card.modal
    }

    /// Sets the modality intent and repaints when it changes.
    pub fn set_modal(&mut self, modal: bool) {
        if self.card.modal == modal {
            return;
        }
        self.card.modal = modal;
        self.base.request_redraw();
    }

    /// The caller-supplied severity glyph, or `None` for the information mark.
    pub fn icon(&self) -> Option<IconName> {
        self.card.icon
    }

    /// Replaces the glyph and repaints. `None` restores the information mark.
    pub fn set_icon(&mut self, icon: Option<IconName>) {
        if self.card.icon == icon {
            return;
        }
        self.card.icon = icon;
        self.base.request_redraw();
    }

    /// The informative text painted below the message.
    pub fn informative_text(&self) -> &str {
        &self.card.informative_text
    }

    /// Replaces the informative text and repaints.
    pub fn set_informative_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.informative_text == text {
            return;
        }
        self.card.informative_text = text;
        self.base.request_redraw();
    }

    /// The detailed text painted below the informative line.
    pub fn detailed_text(&self) -> &str {
        &self.card.detailed_text
    }

    /// Replaces the detailed text and repaints.
    pub fn set_detailed_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.card.detailed_text == text {
            return;
        }
        self.card.detailed_text = text;
        self.base.request_redraw();
    }

    /// The action Enter activates.
    pub fn default_button(&self) -> usize {
        self.card.default_button
    }

    /// Sets the action Enter activates and repaints when it changes.
    pub fn set_default_button(&mut self, index: usize) {
        if self.card.default_button == index {
            return;
        }
        self.card.default_button = index;
        self.base.request_redraw();
    }

    /// The optional check box row's label, or `""` when none is painted.
    pub fn check_box_text(&self) -> &str {
        &self.card.check_box_text
    }

    /// Installs a check box row above the actions. An empty label removes it.
    pub fn set_check_box(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.card.check_box_text == label {
            return;
        }
        self.card.check_box_text = label;
        self.base.request_redraw();
    }

    /// Whether the check box is checked.
    pub fn is_check_box_checked(&self) -> bool {
        self.card.check_box_checked
    }

    /// Sets the check box state and repaints when it changes.
    pub fn set_check_box_checked(&mut self, checked: bool) {
        if self.card.check_box_checked == checked {
            return;
        }
        self.card.check_box_checked = checked;
        self.base.request_redraw();
    }

    /// How far the card has revealed.
    pub fn reveal_progress(&self) -> f32 {
        self.card.reveal()
    }

    /// Advances the reveal and reports whether another frame is needed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    /// Whether the reveal is moving.
    pub fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    /// The dialog's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the dialog's title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// The application's name.
    pub fn application_name(&self) -> &str {
        &self.application_name
    }

    /// Replaces the application's name.
    pub fn set_application_name(&mut self, name: String) {
        if self.application_name == name {
            return;
        }
        self.application_name = name;
        self.base.request_redraw();
    }

    /// The application's version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Replaces the application's version.
    pub fn set_version(&mut self, version: String) {
        if self.version == version {
            return;
        }
        self.version = version;
        self.base.request_redraw();
    }

    /// The descriptive message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces the descriptive message.
    pub fn set_message(&mut self, message: String) {
        if self.message == message {
            return;
        }
        self.message = message;
        self.base.request_redraw();
    }

    /// The two identity lines painted above the message.
    fn identity_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(2);
        if !self.application_name.is_empty() {
            lines.push(self.application_name.clone());
        }
        if !self.version.is_empty() {
            lines.push(std::format!("Version {}", self.version));
        }
        lines
    }

    /// The action row's rects.
    fn buttons(&self) -> Vec<Rect> {
        measured_buttons(self.geometry(), "about_dialog", &self.base, &["Close"])
    }

    /// The check box row's rect, when a row is installed.
    fn check_box_rect(&self) -> Option<Rect> {
        if self.card.check_box_text.is_empty() {
            return None;
        }
        let row_h = BUTTON_HEIGHT.min(self.geometry().height);
        let row_y = self
            .buttons()
            .first()
            .map(|first| first.y.saturating_sub(row_h as i32).saturating_sub(4))
            .unwrap_or_else(|| {
                self.geometry().y + self.geometry().height as i32
                    - CARD_PADDING as i32
                    - row_h as i32
            });
        Some(Rect::new(
            self.geometry().x + CARD_PADDING as i32,
            row_y,
            self.geometry().width.saturating_sub(2 * CARD_PADDING),
            row_h,
        ))
    }
}

impl Widget for AboutDialog {
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
        self.card.preferred_size
    }

    fn show(&mut self) {
        self.base.show();
        self.card.aim(true);
        self.base.request_redraw();
    }

    fn hide(&mut self) {
        self.base.hide();
        self.card.aim(false);
        self.base.request_redraw();
    }

    fn tick(&mut self, delta_ms: u32) -> bool {
        self.card.tick(self.base.is_visible(), delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.card.is_animating()
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for AboutDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "application_name" => Ok(CapabilityValue::String(self.application_name.clone())),
            "version" => Ok(CapabilityValue::String(self.version.clone())),
            "message" => Ok(CapabilityValue::String(self.message.clone())),
            "informative_text" => Ok(CapabilityValue::String(self.informative_text().to_string())),
            "detailed_text" => Ok(CapabilityValue::String(self.detailed_text().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "default_button" => Ok(CapabilityValue::UInt(self.default_button() as u64)),
            "check_box" => Ok(CapabilityValue::String(self.check_box_text().to_string())),
            "check_box_checked" => Ok(CapabilityValue::Bool(self.is_check_box_checked())),
            "icon" => Ok(CapabilityValue::String(
                self.icon().map(|icon| icon.as_str().to_string()).unwrap_or_default(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "application_name" => {
                self.set_application_name(expect_string(value)?);
                Ok(())
            }
            "version" => {
                self.set_version(expect_string(value)?);
                Ok(())
            }
            "message" => {
                self.set_message(expect_string(value)?);
                Ok(())
            }
            "informative_text" => {
                self.set_informative_text(expect_string(value)?);
                Ok(())
            }
            "detailed_text" => {
                self.set_detailed_text(expect_string(value)?);
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "default_button" => {
                let index = match value {
                    CapabilityValue::UInt(v) => v as usize,
                    CapabilityValue::Int(v) if v >= 0 => v as usize,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_default_button(index);
                Ok(())
            }
            "check_box" => {
                self.set_check_box(expect_string(value)?);
                Ok(())
            }
            "check_box_checked" => {
                self.set_check_box_checked(expect_bool(value)?);
                Ok(())
            }
            "icon" => {
                let token = expect_string(value)?;
                let icon = if token.is_empty() {
                    None
                } else {
                    Some(IconName::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?)
                };
                self.set_icon(icon);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "application_name",
            "version",
            "message",
            "informative_text",
            "detailed_text",
            "modal",
            "default_button",
            "check_box",
            "check_box_checked",
            "icon",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for AboutDialog {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let lines = self.identity_lines();
        let content = CardContent {
            role: "about_dialog",
            title: &self.title,
            message: &self.message,
            informative: &self.card.informative_text,
            detailed: &self.card.detailed_text,
            extra_lines: &lines,
            buttons: &["Close"],
            hovered: self.hovered,
            focused: None,
            icon: self.card.icon,
            check_box: (!self.card.check_box_text.is_empty())
                .then_some((self.card.check_box_text.as_str(), self.card.check_box_checked)),
            modal: self.card.modal,
            reveal: self.card.reveal(),
            wrap_cache: &self.card.wrap_cache,
            default_button: Some(self.card.default_button),
        };
        let _ = paint_card(ctx, &self.base, &content);
    }
}

impl EventHandler for AboutDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let hovered = button_at(&self.buttons(), *pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if button_at(&self.buttons(), *pos).is_some() {
                    self.closed.emit();
                    self.base.request_redraw();
                    return;
                }
                if self.check_box_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.set_check_box_checked(!self.card.check_box_checked);
                }
            }
            Event::KeyPress { key, .. }
                if *key == key_codes::ENTER
                    || *key == key_codes::LINE_FEED
                    || *key == key_codes::ESCAPE =>
            {
                self.closed.emit();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    /// Records how many times a payload-free signal fired.
    fn counter(signal: &GenericSignal) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        signal.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        seen
    }

    #[test]
    fn an_alert_acknowledges_through_the_property_independent_path() {
        let mut dialog =
            AlertDialog::new("Heads up", "Something happened.", Rect::new(0, 0, 320, 160));
        let fired = counter(&dialog.accepted);
        let rect = dialog.buttons()[0];
        dialog.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 1, rect.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(fired.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(dialog.kind(), WidgetKind::AlertDialog);
    }

    #[test]
    fn an_alert_press_outside_the_action_does_nothing() {
        let mut dialog = AlertDialog::new("Heads up", "Body", Rect::new(0, 0, 320, 160));
        let fired = counter(&dialog.accepted);
        dialog.handle_event(&Event::MousePress { pos: Point::new(4, 4), button: 1, modifiers: 0 });
        assert_eq!(fired.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn a_confirm_dialog_routes_each_action_to_its_own_signal() {
        let mut dialog =
            ConfirmDialog::new("Delete?", "This cannot be undone.", Rect::new(0, 0, 360, 170));
        let accepted = counter(&dialog.accepted);
        let rejected = counter(&dialog.rejected);
        let rects = dialog.buttons();
        assert_eq!(rects.len(), 2);

        dialog.handle_event(&Event::MousePress {
            pos: Point::new(rects[0].x + 1, rects[0].y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(rejected.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 0);

        dialog.handle_event(&Event::MousePress {
            pos: Point::new(rects[1].x + 1, rects[1].y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn the_action_row_is_laid_out_right_to_left_without_mirroring() {
        let dialog = ConfirmDialog::new("q", "m", Rect::new(0, 0, 360, 170));
        let rects = dialog.buttons();
        assert!(rects[0].x < rects[1].x, "reject is left of accept: {rects:?}");
        // Both buttons are inside the card.
        for rect in &rects {
            assert!(rect.x >= 0 && rect.x + rect.width as i32 <= 360);
        }
    }

    #[test]
    fn an_about_dialog_closes_and_shows_its_identity() {
        let mut dialog = AboutDialog::new("Widgets", "2.9.0", Rect::new(0, 0, 360, 200));
        let fired = counter(&dialog.closed);
        assert_eq!(
            dialog.identity_lines(),
            vec!["Widgets".to_string(), "Version 2.9.0".to_string()]
        );
        let rect = dialog.buttons()[0];
        dialog.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 1, rect.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(fired.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn every_dialog_field_round_trips_through_the_property_route() {
        let mut alert = AlertDialog::new("", "", Rect::new(0, 0, 320, 160));
        alert.set("title", CapabilityValue::String("T".to_string())).expect("writable");
        alert.set("message", CapabilityValue::String("M".to_string())).expect("writable");
        alert.set("button_text", CapabilityValue::String("Got it".to_string())).expect("writable");
        assert_eq!(alert.get("button_text"), Ok(CapabilityValue::String("Got it".to_string())));

        let mut confirm = ConfirmDialog::new("", "", Rect::new(0, 0, 360, 170));
        confirm.set("accept_text", CapabilityValue::String("Yes".to_string())).expect("writable");
        confirm.set("reject_text", CapabilityValue::String("No".to_string())).expect("writable");
        assert_eq!(confirm.get("accept_text"), Ok(CapabilityValue::String("Yes".to_string())));

        let mut about = AboutDialog::new("", "", Rect::new(0, 0, 360, 200));
        about
            .set("application_name", CapabilityValue::String("App".to_string()))
            .expect("writable");
        about.set("version", CapabilityValue::String("1.0".to_string())).expect("writable");
        assert_eq!(about.get("version"), Ok(CapabilityValue::String("1.0".to_string())));
    }

    #[test]
    fn a_message_wraps_on_word_boundaries() {
        assert_eq!(
            wrap_words("one two three", 7),
            vec!["one two".to_string(), "three".to_string()]
        );
        assert_eq!(wrap_words("short", 20), vec!["short".to_string()]);
        // A word longer than the line is hard-split rather than dropped.
        assert_eq!(
            wrap_words("abcdefgh", 3),
            vec!["abc".to_string(), "def".to_string(), "gh".to_string()]
        );
        assert!(wrap_words("anything", 0).is_empty());
    }

    #[test]
    fn every_dialog_paints_its_title_and_action() {
        let mut alert = AlertDialog::new("Notice", "Body text", Rect::new(0, 0, 320, 160));
        let svg = crate::widget::svg::render_to_svg(&mut alert);
        // Title, one message line and one button label.
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(boxes.len(), 3, "{svg}");
        the_severity_glyph_leads_the_row(&svg);

        let mut confirm = ConfirmDialog::new("Q", "M", Rect::new(0, 0, 360, 170));
        let svg = crate::widget::svg::render_to_svg(&mut confirm);
        // Title, one message line and two button labels.
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(boxes.len(), 4, "{svg}");
        the_severity_glyph_leads_the_row(&svg);

        let mut about = AboutDialog::new("App", "1.0", Rect::new(0, 0, 360, 200));
        let svg = crate::widget::svg::render_to_svg(&mut about);
        // Title, name, version and button.
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(boxes.len(), 4, "{svg}");
        the_severity_glyph_leads_the_row(&svg);
    }

    /// The severity glyph is the only non-text path in a card's document: every painted string is
    /// a glyph path marked `data-text`, the icon outline is not, and the card's own chrome is
    /// rects. One path therefore means the glyph sits beside the title, at the card's top-left.
    fn the_severity_glyph_leads_the_row(svg: &str) {
        let glyph_paths = svg
            .lines()
            .filter(|line| line.contains("<path") && !line.contains("data-text"))
            .count();
        assert_eq!(glyph_paths, 1, "exactly one icon path expected: {svg}");
    }

    #[test]
    fn each_dialog_severity_maps_to_its_own_glyph() {
        assert_eq!(CardIcon::Warning.name(), IconName::Warning);
        assert_eq!(CardIcon::Question.name(), IconName::Help);
        assert_eq!(CardIcon::Info.name(), IconName::Info);
    }

    #[test]
    fn a_severity_falls_back_to_a_readable_literal_without_a_theme() {
        // The literals stand in for the semantic tokens only when no theme resolves them; they
        // must stay distinct so a warning never reads as an information mark.
        assert_ne!(CardIcon::Warning.color_fallback(), CardIcon::Info.color_fallback());
        assert_eq!(CardIcon::Question.color_fallback(), CardIcon::Info.color_fallback());
    }

    #[test]
    fn informative_and_detailed_text_add_their_own_lines() {
        let mut dialog = AlertDialog::new("Notice", "Body", Rect::new(0, 0, 340, 220));
        let before =
            crate::widget::svg::text_ink_boxes(&crate::widget::svg::render_to_svg(&mut dialog))
                .len();
        dialog.set_informative_text("Details that qualify the message");
        dialog.set_detailed_text("A further explanation");
        let after =
            crate::widget::svg::text_ink_boxes(&crate::widget::svg::render_to_svg(&mut dialog))
                .len();
        assert!(after > before, "the two extra levels must paint: {before} -> {after}");
        assert_eq!(dialog.informative_text(), "Details that qualify the message");
        assert_eq!(dialog.detailed_text(), "A further explanation");
    }

    #[test]
    fn a_custom_icon_replaces_the_role_glyph() {
        let mut dialog = AlertDialog::new("Notice", "Body", Rect::new(0, 0, 320, 160));
        assert_eq!(dialog.icon(), None, "the role's glyph is the default");
        dialog.set_icon(Some(IconName::Info));
        assert_eq!(dialog.icon(), Some(IconName::Info));
        dialog.set_icon(None);
        assert_eq!(dialog.icon(), None);
    }

    #[test]
    fn enter_activates_and_escape_rejects_an_alert() {
        let mut dialog = AlertDialog::new("Notice", "Body", Rect::new(0, 0, 320, 160));
        let accepted = counter(&dialog.accepted);
        let rejected = counter(&dialog.rejected);
        dialog.handle_event(&Event::key_press(key_codes::ENTER, 0));
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
        dialog.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert_eq!(rejected.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn the_check_box_row_toggles_when_pressed() {
        let mut dialog = AlertDialog::new("Notice", "Body", Rect::new(0, 0, 340, 200));
        dialog.set_check_box("Remember my answer");
        assert!(!dialog.is_check_box_checked());
        let row = dialog.check_box_rect().expect("the row is installed");
        dialog.handle_event(&Event::MousePress {
            pos: Point::new(row.x + 2, row.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert!(dialog.is_check_box_checked());
    }

    #[test]
    fn showing_a_notice_card_reveals_it() {
        let mut dialog = AlertDialog::new("Notice", "Body", Rect::new(0, 0, 320, 160));
        assert_eq!(dialog.reveal_progress(), 1.0);
        dialog.hide();
        while dialog.tick(1000) {}
        assert_eq!(dialog.reveal_progress(), 0.0, "the exit settles first");
        dialog.show();
        assert!(dialog.is_animating(), "a re-shown card owes frames");
        assert!(dialog.tick(16));
        let mid = dialog.reveal_progress();
        assert!(mid > 0.0 && mid < 1.0, "the card must pass through an interior reveal: {mid}");
        while dialog.tick(1000) {}
        assert_eq!(dialog.reveal_progress(), 1.0);
    }
}
