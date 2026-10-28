// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FontPreview widget — a font preview panel for font selection dialogs.
//!
//! Displays a preview of a font family at configurable sizes, with sample
//! text, alphabet samples, and pangrams. Supports bold and italic styles.

use core::fmt::Write as _;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_f64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default pangram used for font preview.
const DEFAULT_PANGRAM: &str = "The quick brown fox jumps over the lazy dog";

/// Alphabet sample string.
const ALPHABET_SAMPLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz";

/// Digit sample string.
const DIGIT_SAMPLE: &str = "0123456789";

/// Top padding inside the panel.
const PANEL_TOP: i32 = 10;
/// Height of the info line's box.
const INFO_LINE_HEIGHT: i32 = 18;
/// Gap between the info line and the preview.
const INFO_GAP: i32 = 8;
/// Gap above the separator rule.
const SEPARATOR_BEFORE: i32 = 4;
/// Gap below the separator rule.
const SEPARATOR_AFTER: i32 = 10;
/// Vertical pitch of one sample row.
const SAMPLE_STEP: i32 = 22;
/// Pixels one wheel line scrolls.
const SCROLL_STEP: i32 = SAMPLE_STEP;
/// Width of the scrollbar hint.
const SCROLLBAR_WIDTH: u32 = 4;
/// The weight axis shown when the host has not supplied a family's own weights.
///
/// The crate's [`Font`] models regular and bold, so those are the two tokens that
/// are always meaningful for it. A host that knows a variable family's real axis
/// writes it through [`FontPreview::set_font_weights`].
const DEFAULT_WEIGHT_AXIS: [u16; 2] = [Font::REGULAR_WEIGHT, Font::BOLD_WEIGHT];

/// A font preview widget that displays a font family at configurable sizes
/// with sample text, alphabet samples, and pangrams.
pub struct FontPreview {
    base: BaseWidget,
    font_family: String,
    font_size: f32,
    preview_text: String,
    sample_texts: Vec<String>,
    bold: bool,
    italic: bool,
    /// Whether the weight-axis strip is drawn. Off by default, so the resting
    /// panel keeps its previous pixels.
    show_weights: bool,
    /// The selected family's available weights, in the order the host supplied.
    /// Empty means "use [`DEFAULT_WEIGHT_AXIS`]".
    font_weights: Vec<u16>,
    /// Vertical scroll offset of the specimen, in pixels, clamped to the content.
    scroll_offset: i32,
    /// An in-progress pointer drag: the press position and the offset it started from.
    drag: Option<(Point, i32)>,
    /// Faces built from the configured family, keyed by `f32::to_bits` of their size.
    ///
    /// `build_font` allocates the family name on every call, so a panel that repaints on
    /// scroll rebuilt the same one or two faces per frame. The cache is cleared whenever
    /// the family, the bold flag or the italic flag changes. Entries are kept in least-recently
    /// used order so a full cache evicts **one** sample rather than clearing every face.
    font_cache: Vec<(u32, Font)>,
    /// The info line and the inputs that produced it.
    info_text: String,
    info_key: Option<(String, u32, bool, bool)>,
    /// The weight-axis rows and the inputs that produced them.
    weight_lines: Vec<String>,
    weight_key: Option<(Vec<u16>, String)>,
}

impl FontPreview {
    /// Creates a new FontPreview widget with the given geometry.
    pub fn new(font_family: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FontPreview, geometry, "FontPreview"),
            font_family: font_family.to_string(),
            font_size: 24.0,
            preview_text: DEFAULT_PANGRAM.to_string(),
            sample_texts: vec![
                DEFAULT_PANGRAM.to_string(),
                ALPHABET_SAMPLE.to_string(),
                DIGIT_SAMPLE.to_string(),
            ],
            bold: false,
            italic: false,
            show_weights: false,
            font_weights: Vec::new(),
            scroll_offset: 0,
            drag: None,
            font_cache: Vec::new(),
            info_text: String::new(),
            info_key: None,
            weight_lines: Vec::new(),
            weight_key: None,
        }
    }

    /// Sets the font family name.
    pub fn set_font_family(&mut self, name: &str) {
        if self.font_family == name {
            return;
        }
        self.font_family = name.to_string();
        self.font_cache.clear();
        self.base.request_redraw();
    }

    /// Returns the font family name.
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Sets the font size for the main preview text.
    pub fn set_font_size(&mut self, size: f32) {
        let size = size.max(4.0);
        if self.font_size == size {
            return;
        }
        self.font_size = size;
        self.clamp_scroll_offset();
        self.base.request_redraw();
    }

    /// Returns the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Sets the preview text displayed at the top.
    pub fn set_preview_text(&mut self, text: &str) {
        if self.preview_text == text {
            return;
        }
        self.preview_text = text.to_string();
        self.clamp_scroll_offset();
        self.base.request_redraw();
    }

    /// Returns the current preview text.
    pub fn preview_text(&self) -> &str {
        &self.preview_text
    }

    /// Sets whether the font is bold.
    pub fn set_bold(&mut self, bold: bool) {
        if self.bold != bold {
            self.bold = bold;
            self.font_cache.clear();
            self.base.request_redraw();
        }
    }

    /// Returns whether the font is bold.
    pub fn is_bold(&self) -> bool {
        self.bold
    }

    /// Sets whether the font is italic.
    pub fn set_italic(&mut self, italic: bool) {
        if self.italic != italic {
            self.italic = italic;
            self.font_cache.clear();
            self.base.request_redraw();
        }
    }

    /// Returns whether the font is italic.
    pub fn is_italic(&self) -> bool {
        self.italic
    }

    /// Returns whether the weight-axis preview strip is enabled.
    pub fn show_weights(&self) -> bool {
        self.show_weights
    }

    /// Enables or disables the weight-axis preview strip.
    ///
    /// When enabled **and** the selected family has more than one available weight,
    /// the panel draws one sample line per weight token below the ordinary samples.
    /// Off by default, so a caller that does not opt in sees the previous panel
    /// unchanged.
    pub fn set_show_weights(&mut self, on: bool) {
        if self.show_weights == on {
            return;
        }
        self.show_weights = on;
        self.clamp_scroll_offset();
        self.base.request_redraw();
    }

    /// Returns the weight tokens the preview treats as available.
    ///
    /// The host-supplied list when one was written, otherwise
    /// `DEFAULT_WEIGHT_AXIS`.
    pub fn font_weights(&self) -> &[u16] {
        if self.font_weights.is_empty() {
            &DEFAULT_WEIGHT_AXIS
        } else {
            &self.font_weights
        }
    }

    /// Sets the selected family's available weights.
    ///
    /// Values are normalised to the CSS 100..=900 hundreds, sorted, and de-duplicated.
    /// An empty list restores the default regular/bold axis. The renderer in this
    /// crate does not enumerate a font family's faces, so the host is the only party
    /// that can answer this question for a variable family; this write is how its
    /// answer reaches the strip.
    pub fn set_font_weights(&mut self, weights: Vec<u16>) {
        let mut weights: Vec<u16> = weights.into_iter().map(Font::normalize_weight).collect();
        weights.sort_unstable();
        weights.dedup();
        if self.font_weights == weights {
            return;
        }
        self.font_weights = weights;
        self.clamp_scroll_offset();
        self.base.request_redraw();
    }

    /// Returns the specimen's vertical scroll offset in pixels.
    pub fn scroll_offset(&self) -> i32 {
        self.scroll_offset
    }

    /// Sets the specimen's vertical scroll offset, clamped to the content.
    ///
    /// The setter is public so a host with its own scrollbar can drive the same
    /// offset the pointer and wheel paths produce. A value past either end is
    /// clamped rather than wrapped.
    pub fn set_scroll_offset(&mut self, offset: i32) {
        let offset = offset.clamp(0, self.max_scroll_offset());
        if self.scroll_offset == offset {
            return;
        }
        self.scroll_offset = offset;
        self.base.request_redraw();
    }

    /// The height the specimen would occupy at its natural layout.
    fn content_height(&self) -> i32 {
        let mut height = PANEL_TOP + INFO_LINE_HEIGHT + INFO_GAP;
        if !self.preview_text.is_empty() {
            height += (self.font_size * 1.4) as i32;
        }
        height += SEPARATOR_BEFORE + SEPARATOR_AFTER;
        height += self.sample_texts.len() as i32 * SAMPLE_STEP;
        if self.show_weights && self.font_weights().len() > 1 {
            height += self.font_weights().len() as i32 * SAMPLE_STEP;
        }
        height
    }

    /// The largest offset that still shows content.
    fn max_scroll_offset(&self) -> i32 {
        (self.content_height() - self.geometry().height as i32).max(0)
    }

    /// Keeps the offset inside the content after a layout-changing write.
    fn clamp_scroll_offset(&mut self) {
        self.scroll_offset = self.scroll_offset.clamp(0, self.max_scroll_offset());
    }

    /// Sets the offset from a signed number of wheel lines.
    ///
    /// Positive lines mean the content moves down, which is the sign
    /// [`Event::Wheel`] carries; the offset moves the other way because it counts
    /// from the top of the content.
    fn scroll_by_lines(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }
        let next = self.scroll_offset.saturating_sub(lines.saturating_mul(SCROLL_STEP));
        self.set_scroll_offset(next);
    }

    /// Draws a proportional scrollbar hint on the right edge.
    ///
    /// It is a hint rather than a control: the wheel and the pointer move the offset,
    /// and the thumb's length reports how much of the specimen is visible. Nothing is
    /// emitted at all when the content fits, so the resting panel is unchanged.
    fn draw_scrollbar(&self, context: &mut RenderContext, track_color: Color, thumb_color: Color) {
        let max = self.max_scroll_offset();
        let rect = self.geometry();
        if max <= 0 || rect.height <= 4 || rect.width <= SCROLLBAR_WIDTH + 4 {
            return;
        }
        let track = Rect::new(
            rect.x + rect.width as i32 - SCROLLBAR_WIDTH as i32 - 2,
            rect.y + 2,
            SCROLLBAR_WIDTH,
            rect.height - 4,
        );
        let view = rect.height as f32;
        let content = self.content_height().max(1) as f32;
        let thumb_height = ((view * view / content) as u32).clamp(8, track.height);
        let travel = track.height - thumb_height;
        let progress = self.scroll_offset as f32 / max as f32;
        let thumb_y = track.y + (travel as f32 * progress) as i32;
        context.fill_rounded_rect(track, SCROLLBAR_WIDTH / 2, track_color);
        context.fill_rounded_rect(
            Rect::new(track.x, thumb_y, track.width, thumb_height),
            SCROLLBAR_WIDTH / 2,
            thumb_color,
        );
    }

    /// Returns the sample texts.
    pub fn sample_texts(&self) -> &[String] {
        &self.sample_texts
    }

    /// Sets the sample texts for the preview.
    pub fn set_sample_texts(&mut self, texts: Vec<String>) {
        if self.sample_texts == texts {
            return;
        }
        self.sample_texts = texts;
        self.clamp_scroll_offset();
        self.base.request_redraw();
    }

    /// Builds a font from current settings.
    fn build_font(&self, size: f32) -> Font {
        Font::new(&self.font_family, size, self.bold, self.italic)
    }

    /// Returns the configured face at `size`, reusing a construction from an earlier call.
    ///
    /// Cloning the cached value only bumps the family reference count; it does not allocate
    /// the family name again. A derived weight is still applied by the caller with
    /// `with_weight_value`, which is likewise a cheap clone.
    fn cached_font(&mut self, size: f32) -> Font {
        // Only the preview size and the two 14 pt specimen faces are ever requested, so this
        // bound is never hit by normal drawing. It keeps a host that animates `font_size`
        // from growing the cache one entry per frame.
        const MAX_CACHED_SIZES: usize = 8;

        let size_bits = size.to_bits();
        if let Some(index) = self.font_cache.iter().position(|(bits, _)| *bits == size_bits) {
            // Promote the hit to the most-recently-used end, so eviction below removes the
            // face that has gone longest without being asked for.
            let entry = self.font_cache.remove(index);
            let font = entry.1.clone();
            self.font_cache.push(entry);
            return font;
        }
        if self.font_cache.len() >= MAX_CACHED_SIZES {
            // Evict one sample, not the whole cache: clearing it made every visible row
            // re-measure after a single new size was requested.
            self.font_cache.remove(0);
        }
        let font = self.build_font(size);
        self.font_cache.push((size_bits, font.clone()));
        font
    }

    /// The info line and the inputs that produced it.
    fn ensure_info_text(&mut self) {
        let key = (self.font_family.clone(), self.font_size.to_bits(), self.bold, self.italic);
        if self.info_key.as_ref() == Some(&key) {
            return;
        }
        self.info_text.clear();
        let _ = write!(
            self.info_text,
            "{} ({}pt{}{})",
            self.font_family,
            self.font_size as u32,
            if self.bold { ", Bold" } else { "" },
            if self.italic { ", Italic" } else { "" },
        );
        self.info_key = Some(key);
    }

    /// The weight-axis rows and the inputs that produced them.
    fn ensure_weight_lines(&mut self) {
        if !self.show_weights || self.font_weights().len() <= 1 {
            return;
        }
        let weights = self.font_weights().to_vec();
        let specimen = if self.preview_text.is_empty() {
            DEFAULT_PANGRAM.to_string()
        } else {
            self.preview_text.clone()
        };
        let key = (weights.clone(), specimen.clone());
        if self.weight_key.as_ref() == Some(&key) {
            return;
        }
        self.weight_lines.clear();
        for weight in weights {
            let mut line = String::new();
            let _ = write!(line, "{weight}  {specimen}");
            self.weight_lines.push(line);
        }
        self.weight_key = Some(key);
    }

    /// The fixed info-line face. Built once through `OnceLock`: `Font` owns its family
    /// as a `String`, so a constructor call per frame was a fresh allocation for a face
    /// that never changes (the same arrangement the emoji picker's chrome faces use).
    /// The configured font in [`Self::build_font`] stays per-call — it carries the
    /// caller's family and varies with the properties.
    fn info_font() -> &'static Font {
        static FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        FONT.get_or_init(|| Font::new("sans-serif", 12.0, false, false))
    }

    /// The panel fill and the three inks its rows use, in precedence order:
    /// the control's explicit style, then the active theme's roles, then literals.
    ///
    /// # Why one resolver
    ///
    /// This panel used five literals and read the theme nowhere, so on a dark appearance it painted
    /// a light rectangle with black ink that could not be reached by any theme. Resolving them
    /// together keeps the four in step: a caller that sets only `background_color` still gets inks
    /// that are legible on it, and a themed build moves all four at once. The literals are the last
    /// resort for a build with no theme at all — the same precedence `WidgetStyle` uses.
    pub(crate) fn panel_colors(&self) -> (Color, Color, Color, Color) {
        let style = self.style();
        // The guard is released before `self.style()` is read below: `theme_manager()` is a
        // non-reentrant mutex, so the theme facts are copied out of it in one scope and the rest of
        // the resolution happens without holding it.
        let themed = crate::style::palette_shared().map(|theme| {
            (theme.surface_container, theme.foreground, theme.secondary, theme.outline_variant)
        });
        let (t_surface, t_ink, t_muted, t_separator) = themed.unwrap_or((
            Color::WHITE,
            Color::BLACK,
            Color::GRAY,
            Color::rgba(200, 200, 200, 255),
        ));

        let panel = style.background_color.unwrap_or(t_surface);
        let ink = style.text_color.unwrap_or(t_ink);
        // A sample row and the info line are annotations about the preview, not the preview
        // itself, so they take the theme's weak ink rather than a second `foreground` literal.
        let muted = t_muted;
        // A divider between the preview and the samples is structure, not content, so it is the
        // weak separator role rather than a grey about the same strength as the sample text.
        let separator = style.border_color.unwrap_or(t_separator);
        (panel, ink, muted, separator)
    }
}

impl Widget for FontPreview {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::FONT_PREVIEW_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::FONT_PREVIEW_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FontPreview`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FontPreview {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "font_family" => Ok(CapabilityValue::String(self.font_family().to_string())),
            "font_size" => Ok(CapabilityValue::Float(f64::from(self.font_size()))),
            "preview_text" => Ok(CapabilityValue::String(self.preview_text().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "font_family" => {
                self.set_font_family(&expect_string(value)?);
                Ok(())
            }
            "font_size" => {
                self.set_font_size(expect_f64(value)? as f32);
                Ok(())
            }
            "preview_text" => {
                self.set_preview_text(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["font_family", "font_size", "preview_text", BASE_PROPERTY_NAMES]
    }
}

impl Draw for FontPreview {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Every colour below used to be a literal (a white panel, black preview, two greys and a
        // separator grey), so the control painted a light panel on any appearance and could not be
        // themed at all — `audit_appearance.py` counted it among the Draw files reading no style.
        // They now resolve from one place: an explicit style first, then the theme's roles, then the
        // literals as a last resort for a build with no theme at all.
        let (panel, ink, muted, separator) = self.panel_colors();
        context.face_with_gradient_corners(
            rect,
            panel,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        let mut y: i32 = rect.y + PANEL_TOP - self.scroll_offset;
        let margin = 10u32;
        // A scrollbar is drawn at the right edge only when the specimen is taller than
        // the panel, so the text budget gives it room only in that case. A resting panel
        // with short content keeps the previous `rect.width - 20` budget exactly.
        let scrollbar_visible = self.max_scroll_offset() > 0;
        let text_w = rect
            .width
            .saturating_sub(margin * 2 + if scrollbar_visible { SCROLLBAR_WIDTH + 2 } else { 0 });
        // Every string in this panel is a *sample* of the configured font, not a label:
        // a pangram is 43 characters whatever the widget is wide, and at the default 24 pt
        // it advances ten times the control's width. The budget is therefore the panel's own
        // inner width, shared by all three runs so the info line, the preview and the sample
        // rows are truncated by the same rule instead of each drifting past the right edge.
        let clipped = self.scroll_offset > 0;
        if clipped {
            // Only a scrolled specimen needs a clip: at offset zero the sample rows still
            // stop at the panel's foot, which keeps the resting frame byte-identical.
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
        }

        // --- Font family name and style info ---
        let info_font = Self::info_font();
        self.ensure_info_text();
        // Fitted to the panel's inner width: the info line carries the family, the size and
        // any style suffix, so it grows with the caller's own inputs and can outrun the panel
        // just as the samples can. It painted in the same grey as a sample row, so the two
        // overflows were indistinguishable in the snapshot.
        context.draw_text_fitted_cow(
            Rect::new(rect.x + margin as i32, y, text_w, info_font.size() as u32),
            self.info_text.as_str(),
            info_font,
            muted,
            HorizontalAlignment::Left,
        );

        y += INFO_LINE_HEIGHT + INFO_GAP;

        // --- Preview text at configured size ---
        if !self.preview_text.is_empty() {
            // Truncate the sample rather than shrink the type: the preview's whole purpose is
            // to show the family at `font_size`, and a font that silently rendered smaller
            // than the size the panel's own info line reports would be a worse lie than an
            // ellipsis. Fitting is therefore the correct fix here, not scaling.
            let preview_font = self.cached_font(self.font_size);
            context.draw_text_fitted_cow(
                Rect::new(rect.x + margin as i32, y, text_w, preview_font.size() as u32),
                &self.preview_text,
                &preview_font,
                ink,
                HorizontalAlignment::Left,
            );
            y += (self.font_size * 1.4) as i32;
        }

        // --- Separator line ---
        y += SEPARATOR_BEFORE;
        // `y` is already absolute (it starts at `rect.y + PANEL_TOP`); adding `rect.y` again
        // drew the separator one control-height too low, and the same double count in the
        // sample-row budget ended the samples early.
        context.draw_line(
            Point::new(rect.x + margin as i32, y),
            Point::new(rect.x + rect.width as i32 - margin as i32, y),
            separator,
        );
        y += SEPARATOR_AFTER;

        // --- Sample texts ---
        let sample_font = self.cached_font(14.0);
        let bottom = rect.y + rect.height as i32;
        // Vertical culling: rows above the panel are skipped instead of being measured and
        // submitted only to be clipped. `first` is derived from the same `SAMPLE_STEP` the
        // layout advances by.
        let first_visible = ((rect.y - y).max(0) / SAMPLE_STEP) as usize;
        let first_visible = first_visible.min(self.sample_texts.len());
        y += first_visible as i32 * SAMPLE_STEP;
        for text in &self.sample_texts[first_visible..] {
            if !clipped && y + 20 > bottom {
                break; // Don't draw past the widget boundary
            }
            if y + SAMPLE_STEP >= rect.y {
                // The alphabet and the pangram are longer than any panel this widget is sized
                // for, so each row is fitted to the same inner width as the preview above it.
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + margin as i32, y, text_w, sample_font.size() as u32),
                    text,
                    &sample_font,
                    muted,
                    HorizontalAlignment::Left,
                );
            }
            y += SAMPLE_STEP;
        }

        // --- Weight axis ---
        let weight_base_font = self.cached_font(14.0);
        self.ensure_weight_lines();
        let weight_rows = self.font_weights().len();
        if self.show_weights && weight_rows > 1 {
            let first_visible = ((rect.y - y).max(0) / SAMPLE_STEP) as usize;
            let first_visible = first_visible.min(self.weight_lines.len());
            y += first_visible as i32 * SAMPLE_STEP;
            for (offset, line) in self.weight_lines[first_visible..].iter().enumerate() {
                if !clipped && y + 20 > bottom {
                    break;
                }
                let weight = self.font_weights()[first_visible + offset];
                let font = weight_base_font.clone().with_weight_value(weight);
                if y + SAMPLE_STEP >= rect.y {
                    context.draw_text_fitted_cow(
                        Rect::new(rect.x + margin as i32, y, text_w, font.size() as u32),
                        line,
                        &font,
                        muted,
                        HorizontalAlignment::Left,
                    );
                }
                y += SAMPLE_STEP;
            }
        }

        if clipped {
            context.pop_clip();
        }
        // Drawn after the content so the thumb is never covered by a sample row.
        self.draw_scrollbar(context, separator.blend(&panel, 0.6), muted.blend(&panel, 0.2));
    }
}

impl EventHandler for FontPreview {
    /// Scrolls the specimen by wheel or by dragging it vertically.
    ///
    /// The offset is clamped inside [`Self::set_scroll_offset`], so neither gesture
    /// can run past either end of the content. A press inside the panel starts a drag
    /// from the current offset; the drag applies the pointer's vertical travel rather
    /// than a fixed step, which is what makes dragging feel attached to the specimen.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if self.geometry().contains_point(*pos) {
                    self.drag = Some((*pos, self.scroll_offset));
                }
            }
            Event::MouseMove { pos } => {
                if let Some((anchor, start)) = self.drag {
                    self.set_scroll_offset(start + (anchor.y - pos.y));
                } else {
                    self.base.handle_event(event);
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.drag = None;
            }
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                self.scroll_by_lines(scroll.lines_y);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(device_profile, feature = "desktop"))]
    use crate::core::Size;
    #[cfg(all(device_profile, feature = "desktop"))]
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// Renders the panel over `backdrop` and returns its RGBA frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn render_on(fp: &mut FontPreview, size: Size, backdrop: Color) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        fp.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    /// The panel fill follows the appearance instead of a fixed white.
    ///
    /// # The defect this pins
    ///
    /// `Draw` read no style at all: the panel was `Color::WHITE`, the preview black, the info line
    /// and the sample rows two greys, and the separator a third. On a dark appearance the control
    /// therefore painted a bright panel with near-black ink that no theme could reach — the
    /// `audit_appearance.py` count of "Draw files reading no style colour" named it.
    ///
    /// The sample point is in the panel's blank right-hand area, clear of every glyph and of the
    /// separator rule, so the pixel it reads is the panel fill and nothing else.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_panel_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 300, 200);
        let size = Size::new(300, 200);

        let panel_pixel = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::palette_shared()
                .map(|colors| colors.background)
                .expect("a preset is active");
            let mut fp = FontPreview::new("Arial", rect);
            crate::theme::apply_theme_to_widget(&mut fp);
            let frame = render_on(&mut fp, size, backdrop);
            // The right edge of the panel, well below the info line: no sample row reaches it.
            pixel(&frame, size, 295, 150)
        };

        let dark = panel_pixel(crate::theme::AppearanceMode::Dark);
        let light = panel_pixel(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the font-preview panel must follow the appearance; both were {dark:?}"
        );
        assert_ne!(
            dark,
            [255, 255, 255, 255],
            "the panel must not be the fixed white the defect used"
        );
    }

    #[test]
    fn font_preview_initial_state() {
        let fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        assert_eq!(fp.font_family(), "Arial");
        assert!((fp.font_size() - 24.0).abs() < 0.01);
        assert_eq!(fp.preview_text(), DEFAULT_PANGRAM);
        assert!(!fp.is_bold());
        assert!(!fp.is_italic());
        assert_eq!(fp.kind(), WidgetKind::FontPreview);
    }

    #[test]
    fn font_preview_set_font_family() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_font_family("Helvetica");
        assert_eq!(fp.font_family(), "Helvetica");
    }

    #[test]
    fn font_preview_set_font_size() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_font_size(18.0);
        assert!((fp.font_size() - 18.0).abs() < 0.01);

        // Size should clamp to minimum
        fp.set_font_size(0.0);
        assert!((fp.font_size() - 4.0).abs() < 0.01);
    }

    #[test]
    fn font_preview_set_preview_text() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_preview_text("Hello World");
        assert_eq!(fp.preview_text(), "Hello World");

        fp.set_preview_text("");
        assert_eq!(fp.preview_text(), "");
    }

    #[test]
    fn font_preview_bold_italic() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));

        fp.set_bold(true);
        assert!(fp.is_bold());

        fp.set_italic(true);
        assert!(fp.is_italic());

        fp.set_bold(false);
        assert!(!fp.is_bold());
    }

    #[test]
    fn font_preview_sample_texts() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        let samples = fp.sample_texts().to_vec();
        assert_eq!(samples.len(), 3);

        let new_samples = vec!["Sample A".to_string(), "Sample B".to_string()];
        fp.set_sample_texts(new_samples.clone());
        assert_eq!(fp.sample_texts(), &new_samples);
    }

    #[test]
    fn font_preview_build_font() {
        let fp = FontPreview::new("Times New Roman", Rect::new(0, 0, 300, 200));
        let font = fp.build_font(12.0);
        assert_eq!(font.family(), "Times New Roman");
        assert!((font.size() - 12.0).abs() < 0.01);
    }

    /// The weight strip is off by default and adds one sample run per weight token.
    #[test]
    fn font_preview_weight_strip_is_opt_in() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 500));
        assert!(!fp.show_weights());
        assert_eq!(fp.font_weights(), &[Font::REGULAR_WEIGHT, Font::BOLD_WEIGHT]);
        let resting = crate::widget::svg::render_to_svg(&mut fp);
        let resting_runs = crate::widget::svg::text_ink_boxes(&resting).len();

        fp.set_show_weights(true);
        assert!(fp.show_weights());
        let weighted = crate::widget::svg::render_to_svg(&mut fp);
        let weighted_runs = crate::widget::svg::text_ink_boxes(&weighted).len();
        assert!(
            weighted_runs >= resting_runs + 2,
            "one line per default weight token: {resting_runs} -> {weighted_runs}"
        );

        // A family with a single weight has no axis to preview, so the strip disappears
        // even though the switch is on.
        fp.set_font_weights(vec![400]);
        assert_eq!(fp.font_weights(), &[400]);
        let single = crate::widget::svg::render_to_svg(&mut fp);
        assert_eq!(resting, single, "a single weight restores the resting drawing");

        // Host weights are normalised (350 rounds to 400), sorted and de-duplicated.
        fp.set_font_weights(vec![900, 350, 400, 350]);
        assert_eq!(fp.font_weights(), &[400, 900]);
        fp.set_show_weights(false);
        let off = crate::widget::svg::render_to_svg(&mut fp);
        assert_eq!(resting, off, "turning the strip off restores the resting drawing");
    }

    /// The wheel scrolls the specimen and clamps at both ends.
    #[test]
    fn font_preview_wheel_scrolls_and_clamps() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 100));
        let samples: Vec<String> = (0..20).map(|n| format!("Sample {n}")).collect();
        fp.set_sample_texts(samples);
        assert_eq!(fp.scroll_offset(), 0);
        assert!(fp.max_scroll_offset() > 0, "the fixture must be scrollable");

        // A negative delta moves the content up, i.e. reveals the rows below.
        fp.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert_eq!(fp.scroll_offset(), SCROLL_STEP);
        fp.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(fp.scroll_offset(), 0);
        // Spinning past the top clamps rather than wrapping.
        fp.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(fp.scroll_offset(), 0);

        for _ in 0..100 {
            fp.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(fp.scroll_offset(), fp.max_scroll_offset(), "the offset clamps at the content");
    }

    /// A vertical drag moves the specimen by the pointer's own travel.
    #[test]
    fn font_preview_drag_scrolls_and_clamps() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 100));
        let samples: Vec<String> = (0..20).map(|n| format!("Sample {n}")).collect();
        fp.set_sample_texts(samples);

        fp.handle_event(&Event::mouse_press(150, 80, 1));
        fp.handle_event(&Event::mouse_move(150, 50));
        assert_eq!(fp.scroll_offset(), 30, "dragging up 30 px scrolls down 30 px");

        // Dragging back past the start clamps at zero.
        fp.handle_event(&Event::mouse_move(150, 1000));
        assert_eq!(fp.scroll_offset(), 0);

        // A move without a press is not a drag.
        fp.handle_event(&Event::mouse_release(150, 50, 1));
        fp.handle_event(&Event::mouse_move(150, 10));
        assert_eq!(fp.scroll_offset(), 0);
    }

    /// The scrollbar hint is drawn only when the specimen overflows the panel.
    #[test]
    fn font_preview_scrollbar_hint_appears_only_when_needed() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut short = FontPreview::new("Arial", Rect::new(0, 0, 300, 500));
        let short_svg = crate::widget::svg::render_to_svg(&mut short);
        assert!(
            !short_svg.contains("width=\"4\" height="),
            "a specimen that fits has no scrollbar: {short_svg}"
        );

        let mut long = FontPreview::new("Arial", Rect::new(0, 0, 300, 100));
        let samples: Vec<String> = (0..20).map(|n| format!("Sample {n}")).collect();
        long.set_sample_texts(samples);
        let long_svg = crate::widget::svg::render_to_svg(&mut long);
        assert!(
            long_svg.contains("width=\"4\" height="),
            "an overflowing specimen shows the hint: {long_svg}"
        );
    }

    /// The info line and the weight rows are rebuilt only when their inputs change.
    #[test]
    fn the_info_line_and_weight_rows_are_cached() {
        let mut preview = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        preview.ensure_info_text();
        assert_eq!(preview.info_text, "Arial (24pt)");
        let ptr = preview.info_text.as_ptr();
        preview.ensure_info_text();
        assert_eq!(preview.info_text.as_ptr(), ptr, "an unchanged line is reused");

        preview.set_font_size(18.0);
        preview.ensure_info_text();
        assert_eq!(preview.info_text, "Arial (18pt)");

        preview.set_show_weights(true);
        preview.ensure_weight_lines();
        assert_eq!(preview.weight_lines.len(), DEFAULT_WEIGHT_AXIS.len());
        let ptr = preview.weight_lines[0].as_ptr();
        preview.ensure_weight_lines();
        assert_eq!(preview.weight_lines[0].as_ptr(), ptr, "unchanged rows are reused");

        // A different specimen rebuilds every row.
        preview.set_preview_text("Hello");
        preview.ensure_weight_lines();
        assert!(preview.weight_lines[0].ends_with("Hello"));
    }

    /// A full face cache evicts one sample rather than clearing every cached face.
    #[test]
    fn a_full_face_cache_evicts_one_entry() {
        let mut preview = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        for size in 1..=8u32 {
            let _ = preview.cached_font(size as f32);
        }
        assert_eq!(preview.font_cache.len(), 8);
        // A ninth size evicts exactly one entry, not the whole cache.
        let _ = preview.cached_font(9.0);
        assert_eq!(preview.font_cache.len(), 8, "one sample is evicted");
        assert!(preview.font_cache.iter().any(|(bits, _)| *bits == 9.0f32.to_bits()));
    }
}
