// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tag — a compact, tone-coloured label used to categorise or filter content.
//!
//! # Why a tag and not a `Chip`
//!
//! A chip is a **list** control: it owns a collection of items, a focus index and a selection
//! model in one of two modes. A tag is a single label attached to the row it sits in: it
//! carries a semantic tone (neutral / primary / success / warning / error), it can be
//! selected, and it can be closed. Modelling one tag as a one-item chip would give every tag
//! an item list, a focus index and a multi-select flag it can never use.

use crate::compat::{Cell, RefCell};
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{estimate_line_height, estimate_text_width};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The semantic tone of a tag: which of the theme's status colours it is painted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagTone {
    /// The neutral grey, for ordinary labels.
    #[default]
    Neutral,
    /// The brand colour, for the primary categorisation.
    Primary,
    /// The success colour.
    Success,
    /// The warning colour.
    Warning,
    /// The error colour, for a tag the user must notice.
    Error,
}

impl TagTone {
    /// Every tone, in the order they are published.
    pub const ALL: [Self; 5] =
        [Self::Neutral, Self::Primary, Self::Success, Self::Warning, Self::Error];

    /// The token this tone is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Primary => "primary",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tone| tone.token() == token)
    }
}

/// The horizontal padding inside a tag, in logical pixels.
const TAG_PADDING: u32 = 8;
/// The gap between the label and the close glyph.
const TAG_CLOSE_GAP: u32 = 6;
/// The width of the close glyph's own box.
const TAG_CLOSE_SIDE: u32 = 8;
/// The side of the leading icon's slot.
const TAG_ICON_SIDE: u32 = 12;
/// The gap between the leading icon and the label.
const TAG_ICON_GAP: u32 = 4;
/// How far a hovered tag's fill moves towards its contrast colour.
const TAG_HOVER_TINT: f32 = 0.08;

/// Tag — a single tone-coloured label with an optional close glyph.
///
/// The tag reports its own text as its accessible label, and the close glyph is drawn only
/// when `closable` is set — the two are separate properties so a caller can present a tag the
/// user can select but not remove.
///
/// # Removal is the host's job
///
/// Activating the close glyph or pressing Delete/Backspace on a closable tag emits
/// [`Self::closed`]. The tag never removes itself from its parent: it does not own the layout
/// that placed it, so the host that does is the only thing that can remove it.
pub struct Tag {
    base: BaseWidget,
    text: String,
    tone: TagTone,
    selected: bool,
    closable: bool,
    /// An optional leading icon, drawn before the label.
    ///
    /// A tag often carries a category or status mark; without this slot the caller had to
    /// compose an `Icon` beside the tag and own the spacing itself.
    icon: Option<IconName>,
    /// The resolved label face, reused while the style's font is unchanged.
    ///
    /// `draw` used to construct `Font::simple("Arial", 12.0)` per frame for a face that changes
    /// only when the style changes, which is a real allocation on a control that repaints on
    /// every hover.
    font_cache: Option<Font>,
    /// The measured cell and the inputs that produced it.
    ///
    /// The group lays its children out from `size_hint` every frame, and that hint measured
    /// the label on every call. The cell is a function of the text, the resolved face, the
    /// close flag and the optional icon, so it is cached until one of those changes.
    size_hint_cache: RefCell<Option<SizeHintCache>>,
    /// Bumped whenever the measured cell is rebuilt, so `TagGroup` can invalidate its own
    /// layout without re-measuring every child.
    size_hint_revision: Cell<u64>,
    /// Emitted when the close glyph is activated, with the tag's own text as the payload.
    pub closed: Signal1<String>,
    /// Emitted with the new flag when the selection actually changes.
    pub selected_changed: Signal1<bool>,
}

/// The measured cell and the four inputs that produce it.
struct SizeHintCache {
    text: String,
    font: Font,
    closable: bool,
    icon: Option<IconName>,
    size: Size,
}

impl Tag {
    /// Creates a neutral, unselected, non-closable tag labelled `text`.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Tag, geometry, "Tag"),
            text: text.to_string(),
            tone: TagTone::default(),
            selected: false,
            closable: false,
            icon: None,
            font_cache: None,
            size_hint_cache: RefCell::new(None),
            size_hint_revision: Cell::new(0),
            closed: Signal1::new(),
            selected_changed: Signal1::new(),
        }
    }

    /// Compatibility spelling of [`Self::selected_changed`] (W-20).
    ///
    /// A tag's selection is a boolean rather than a current index, so the published
    /// row keeps `selected_changed`; this accessor exposes the canonical event-layer
    /// name and returns the same emitted signal.
    pub fn current_changed(&self) -> &Signal1<bool> {
        &self.selected_changed
    }

    /// The tag's label.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The tag's semantic tone.
    pub fn tone(&self) -> TagTone {
        self.tone
    }

    /// Sets the tag's semantic tone.
    ///
    /// Re-resolves the theme style because the tone selects a different colour role, and a
    /// redraw alone would keep the previously resolved style.
    pub fn set_tone(&mut self, tone: TagTone) {
        if self.tone == tone {
            return;
        }
        self.tone = tone;
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
    }

    /// Whether the tag is selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Sets whether the tag is selected.
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected == selected {
            return;
        }
        self.selected = selected;
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
        self.selected_changed.emit(selected);
    }

    /// Whether the close glyph is drawn and accepted.
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Sets whether the close glyph is drawn and accepted.
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable == closable {
            return;
        }
        self.closable = closable;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the pointer is currently over the tag.
    ///
    /// Read from [`BaseWidget::is_hovered`](crate::widget::base::BaseWidget::is_hovered) rather
    /// than a private copy: the runtime commits the hover transition for every control, so a
    /// second field could answer differently from the state every other control sees
    /// (`check_state_source_is_the_base.sh`).
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }

    /// The tag's optional leading icon.
    pub fn icon(&self) -> Option<IconName> {
        self.icon
    }

    /// Sets the tag's leading icon, or clears it with `None`.
    ///
    /// The icon occupies a fixed slot before the label; it is drawn with the shared
    /// [`crate::widget::display_widgets::icon::draw_icon_at`] primitive, so it follows the tag's
    /// own ink and cannot disagree with an adjacent `Icon` widget.
    pub fn set_icon(&mut self, icon: Option<IconName>) {
        if self.icon == icon {
            return;
        }
        self.icon = icon;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Sets the hover state.
    ///
    /// [`Event::MouseEnter`] and [`Event::MouseLeave`] drive this automatically; the setter
    /// exists for a host that synthesises hover, such as a list forwarding a pointer position.
    pub fn set_hovered(&mut self, hovered: bool) {
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
    }

    /// The rectangle the close glyph occupies, or `None` when the tag is not closable.
    ///
    /// The press test and the painter both call this, so the glyph the user sees and the
    /// target that responds cannot drift apart.
    fn close_rect(&self) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let geometry = self.geometry();
        let side = TAG_CLOSE_SIDE.min(geometry.height).min(geometry.width);
        let x = geometry
            .x
            .saturating_add(geometry.width as i32)
            .saturating_sub(TAG_PADDING as i32)
            .saturating_sub(side as i32);
        let y = geometry.y.saturating_add(((geometry.height - side) / 2) as i32);
        Some(Rect::new(x, y, side, side))
    }

    /// The tone colour this tag resolves from the theme, falling back to its own style.
    ///
    /// One palette snapshot for all five status colours: the previous form took the global theme
    /// manager's lock to read them, which is a second lock acquisition on a control that already
    /// resolved its style this frame.
    fn tone_color(&self, style: &crate::style::WidgetStyle) -> Color {
        let palette = crate::style::palette_shared();
        let (primary, success, warning, error, outline) = match palette.as_ref() {
            Some(colors) => {
                (colors.primary, colors.success, colors.warning, colors.error, colors.outline)
            }
            None => (
                Color::PRIMARY,
                Color::rgb(46, 160, 67),
                Color::rgb(210, 153, 34),
                Color::rgb(207, 34, 46),
                Color::rgb(160, 160, 160),
            ),
        };
        match self.tone {
            TagTone::Neutral => style.border_color.unwrap_or(outline),
            TagTone::Primary => primary,
            TagTone::Success => success,
            TagTone::Warning => warning,
            TagTone::Error => error,
        }
    }

    /// The resolved label face for `style_font`, reusing the cached construction.
    fn resolved_font(&mut self, style_font: Option<Font>) -> Font {
        let reusable = style_font
            .as_ref()
            .is_some_and(|font| self.font_cache.as_ref().is_some_and(|cached| cached == font))
            || (style_font.is_none()
                && self
                    .font_cache
                    .as_ref()
                    .is_some_and(|cached| cached.family() == "Arial" && cached.size() == 12.0));
        if reusable {
            return self.font_cache.clone().unwrap_or_default();
        }
        let font = style_font.unwrap_or_else(|| Font::simple_interned("Arial", 12.0));
        self.font_cache = Some(font.clone());
        font
    }

    /// The revision of this tag's measured cell; see [`Self::size_hint`].
    pub(crate) fn size_hint_revision(&self) -> u64 {
        self.size_hint_revision.get()
    }
}

impl Widget for Tag {
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
        // The label's extent comes from the shared measurement primitives, not a hand-rolled
        // `chars * size`: the flat model this replaced was only correct while every cluster
        // advanced exactly one point size, and diverged the moment the shaper knew more
        // (kerning, wide glyphs, an explicit line height). The font is resolved exactly as
        // `draw` resolves it, so the hint and the painting agree on the same face.
        let font =
            self.base.style().font.clone().unwrap_or_else(|| Font::simple_interned("Arial", 12.0));
        let fresh = self.size_hint_cache.borrow().as_ref().is_some_and(|cache| {
            cache.text == self.text
                && cache.font == font
                && cache.closable == self.closable
                && cache.icon == self.icon
        });
        if !fresh {
            let label = estimate_text_width(&self.text, &font, 1.0);
            let close = if self.closable { TAG_CLOSE_GAP + TAG_CLOSE_SIDE } else { 0 };
            let icon = if self.icon.is_some() { TAG_ICON_SIDE + TAG_ICON_GAP } else { 0 };
            let size = Size::new(
                label.saturating_add(2 * TAG_PADDING).saturating_add(close).saturating_add(icon),
                estimate_line_height(&font, 1.0).saturating_add(8),
            );
            *self.size_hint_cache.borrow_mut() = Some(SizeHintCache {
                text: self.text.clone(),
                font,
                closable: self.closable,
                icon: self.icon,
                size,
            });
            self.size_hint_revision.set(self.size_hint_revision.get().wrapping_add(1));
        }
        self.size_hint_cache
            .borrow()
            .as_ref()
            .map(|cache| cache.size)
            .unwrap_or_else(|| Size::new(0, 0))
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "closed" => Some(EventSignalRef::mapped("closed", &self.closed, |value| {
                CapabilityValue::String(value.clone())
            })),
            "selected_changed" => {
                Some(EventSignalRef::mapped("selected_changed", &self.selected_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Tag {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "tone" => Ok(CapabilityValue::String(self.tone.token().to_string())),
            "selected" => Ok(CapabilityValue::Bool(self.selected)),
            "closable" => Ok(CapabilityValue::Bool(self.closable)),
            "icon" => Ok(CapabilityValue::String(
                self.icon.map_or_else(|| "none".to_string(), |icon| icon.as_str().to_string()),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "tone" => {
                let token = expect_string(value)?;
                let tone = TagTone::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_tone(tone);
                Ok(())
            }
            "selected" => {
                self.set_selected(expect_bool(value)?);
                Ok(())
            }
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "icon" => {
                // The empty spelling clears the slot, because a property editor has no `Null`
                // for a string-typed icon name. An unknown token is refused rather than
                // silently dropped.
                let token = expect_string(value)?;
                if token.is_empty() || token == "none" {
                    self.set_icon(None);
                    return Ok(());
                }
                let icon =
                    IconName::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_icon(Some(icon));
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "tone", "selected", "closable", "icon", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Tag {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style_font = self.base.style().font.clone();
        let font = self.resolved_font(style_font);
        let style = self.base.style();
        let surface = style.background_color.unwrap_or_else(|| {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|theme| theme.colors.background).unwrap_or(Color::WHITE)
        });
        let tone = self.tone_color(style);
        // A selected tag is filled with its tone; an unselected one is a tint of it, so the
        // pair is distinguishable in both appearances without a second hardcoded colour.
        let base_fill = if self.selected { tone } else { tone.blend(&surface, 0.86) };
        // A hovered tag shifts its fill slightly towards its own contrast colour, which
        // lightens it in a dark appearance and darkens it in a light one. At rest the fill is
        // untouched, so the default picture is byte-identical.
        let fill = if self.base.is_hovered() {
            base_fill.blend(&base_fill.contrast_color(), TAG_HOVER_TINT)
        } else {
            base_fill
        };
        let ink = fill.contrast_color();
        let radius = rect.height / 2;
        ctx.fill_rounded_rect(rect, radius, fill);
        ctx.draw_rounded_rect_stroke(rect, radius, tone.with_alpha_f32(0.7), 1);

        let close = self.close_rect();
        let label_right = close.map_or(rect.width, |close| {
            rect.width.saturating_sub(
                close.width.saturating_add(TAG_CLOSE_GAP).saturating_add(TAG_PADDING),
            )
        });
        // The leading icon occupies a fixed slot inside the padding; the label band starts after
        // it, so the two never overlap. The icon is vertically centred and square, matching the
        // shared inline-icon rule.
        let icon_side = TAG_ICON_SIDE.min(rect.height).min(rect.width);
        let mut label_left = rect.x.saturating_add(TAG_PADDING as i32);
        if let Some(icon) = self.icon {
            let icon_y = rect.y.saturating_add(((rect.height - icon_side) / 2) as i32);
            let icon_rect = Rect::new(label_left, icon_y, icon_side, icon_side);
            crate::widget::display_widgets::icon::draw_icon_at(ctx, icon_rect, ink, icon);
            label_left = label_left.saturating_add((icon_side + TAG_ICON_GAP) as i32);
        }
        let label_width = label_right.saturating_sub((label_left - rect.x) as u32);
        let label_band = Rect::new(label_left, rect.y, label_width, rect.height);
        // Bounded text: a long label is fitted with an ellipsis rather than running past the
        // tag (and, in the SVG backend, past the control's own rectangle).
        ctx.draw_text_line_cow(label_band, &self.text, &font, ink, HorizontalAlignment::Left);

        if let Some(close) = close {
            // An "x" drawn as two strokes rather than a glyph: a font-dependent glyph would
            // change the control's ink the moment the theme changed typeface.
            let inset = close.width / 4;
            let from = crate::core::Point::new(
                close.x.saturating_add(inset as i32),
                close.y.saturating_add(inset as i32),
            );
            let to = crate::core::Point::new(
                close.x.saturating_add(close.width as i32).saturating_sub(inset as i32),
                close.y.saturating_add(close.height as i32).saturating_sub(inset as i32),
            );
            ctx.draw_line_stroke(from, to, ink, 1);
            ctx.draw_line_stroke(
                crate::core::Point::new(from.x, to.y),
                crate::core::Point::new(to.x, from.y),
                ink,
                1,
            );
        }
    }
}

impl EventHandler for Tag {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseEnter { .. } => self.set_hovered(true),
            Event::MouseLeave { .. } => self.set_hovered(false),
            // Both removal routes emit the same signal, so a host subscribes once. The tag is
            // only removable when it is closable, and the key is ignored while disabled.
            Event::KeyPress { key, .. }
                if (*key == key_codes::DELETE || *key == key_codes::BACKSPACE)
                    && self.closable
                    && self.base.is_enabled() =>
            {
                self.closed.emit(self.text.clone());
                self.base.request_redraw();
            }
            // A disabled tag accepts no pointer input, exactly like its key arm above (D-024).
            Event::MousePress { pos, button: 1, .. } if self.base.is_enabled() => {
                if let Some(close) = self.close_rect() {
                    if close.contains(*pos) {
                        self.closed.emit(self.text.clone());
                        self.base.request_redraw();
                        return;
                    }
                }
                if self.geometry().contains(*pos) {
                    let selected = !self.selected;
                    self.set_selected(selected);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn a_tag_starts_neutral_and_unselected() {
        let tag = Tag::new("rust", Rect::new(0, 0, 60, 24));
        assert_eq!(tag.tone(), TagTone::Neutral);
        assert!(!tag.is_selected());
        assert!(!tag.is_closable());
        assert_eq!(tag.kind(), WidgetKind::Tag);
    }

    #[test]
    fn a_tag_size_hint_covers_its_label_and_close_glyph() {
        let mut tag = Tag::new("abc", Rect::new(0, 0, 10, 10));
        let without_close = tag.size_hint();
        tag.set_closable(true);
        let with_close = tag.size_hint();
        assert!(with_close.width > without_close.width, "the close glyph takes room");
        assert_eq!(with_close.width - without_close.width, TAG_CLOSE_GAP + TAG_CLOSE_SIDE);
    }

    #[test]
    fn pressing_the_close_glyph_emits_closed_with_the_label() {
        let mut tag = Tag::new("draft", Rect::new(0, 0, 80, 24));
        tag.set_closable(true);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        tag.closed.connect_ref({
            let seen = std::sync::Arc::clone(&seen);
            move |value: &String| seen.lock().expect("lock").push(value.clone())
        });
        let close = tag.close_rect().expect("a closable tag has a close glyph");
        tag.handle_event(&Event::MousePress {
            pos: Point::new(close.x + 1, close.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(seen.lock().expect("lock").as_slice(), ["draft".to_string()]);
        assert!(!tag.is_selected(), "closing must not also toggle selection");
    }

    #[test]
    fn pressing_the_body_toggles_selection() {
        let mut tag = Tag::new("rust", Rect::new(0, 0, 80, 24));
        tag.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(tag.is_selected());
        tag.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(!tag.is_selected());
    }

    #[test]
    fn the_property_route_round_trips_every_field() {
        let mut tag = Tag::new("", Rect::new(0, 0, 80, 24));
        tag.set("text", CapabilityValue::String("beta".to_string())).expect("writable");
        tag.set("tone", CapabilityValue::String("warning".to_string())).expect("writable");
        tag.set("selected", CapabilityValue::Bool(true)).expect("writable");
        tag.set("closable", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(tag.get("text"), Ok(CapabilityValue::String("beta".to_string())));
        assert_eq!(tag.get("tone"), Ok(CapabilityValue::String("warning".to_string())));
        assert_eq!(tag.get("selected"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(tag.get("closable"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(
            tag.set("tone", CapabilityValue::String("chartreuse".to_string())),
            Err(CapabilityAccessError::OutOfRange)
        );
    }

    #[test]
    fn a_tag_paints_its_label() {
        let mut tag = Tag::new("hello", Rect::new(0, 0, 80, 24));
        let svg = crate::widget::svg::render_to_svg(&mut tag);
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1, "{svg}");
    }

    /// Pointer entry and exit drive a hover state, and the fill follows it.
    #[test]
    fn hovering_highlights_the_fill_and_leaving_restores_it() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut tag = Tag::new("rust", Rect::new(0, 0, 80, 24));
        assert!(!tag.is_hovered());
        let rest = crate::widget::svg::render_to_svg(&mut tag);

        tag.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });
        assert!(tag.is_hovered(), "MouseEnter sets the hover state");
        let hovered = crate::widget::svg::render_to_svg(&mut tag);
        assert_ne!(hovered, rest, "a hovered tag paints a different fill: {hovered}");

        tag.handle_event(&Event::MouseLeave { pos: Point::new(200, 200) });
        assert!(!tag.is_hovered(), "MouseLeave clears the hover state");
        let after = crate::widget::svg::render_to_svg(&mut tag);
        assert_eq!(after, rest, "leaving restores the resting fill byte for byte");
    }

    /// Delete and Backspace both request removal through the existing `closed` signal.
    ///
    /// The tag must not remove itself: the host owns the layout entry, so it is the only
    /// thing that can take the tag out.
    #[test]
    fn delete_and_backspace_request_removal_without_removing_the_tag() {
        let mut tag = Tag::new("draft", Rect::new(0, 0, 80, 24));
        tag.set_closable(true);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        tag.closed.connect_ref({
            let seen = std::sync::Arc::clone(&seen);
            move |value: &String| seen.lock().expect("lock").push(value.clone())
        });

        for key in [key_codes::DELETE, key_codes::BACKSPACE] {
            tag.handle_event(&Event::KeyPress { key, modifiers: 0 });
        }

        assert_eq!(
            seen.lock().expect("lock").as_slice(),
            ["draft".to_string(), "draft".to_string()],
            "both keys emit `closed` with the tag's label"
        );
        assert_eq!(tag.text(), "draft", "the host removes the tag, not the tag itself");
        assert!(!tag.is_selected(), "keyboard removal is not a selection change");
    }

    /// A tag the user cannot close is not keyboard-removable either.
    #[test]
    fn a_non_closable_tag_ignores_the_delete_key() {
        let mut tag = Tag::new("pinned", Rect::new(0, 0, 80, 24));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        tag.closed.connect_ref({
            let seen = std::sync::Arc::clone(&seen);
            move |value: &String| seen.lock().expect("lock").push(value.clone())
        });

        tag.handle_event(&Event::KeyPress { key: key_codes::DELETE, modifiers: 0 });
        assert!(seen.lock().expect("lock").is_empty(), "a non-closable tag cannot be removed");
    }

    /// A tag can carry a leading icon, and the slot grows the hint by exactly its own width.
    #[test]
    fn a_tag_can_carry_a_leading_icon() {
        let mut tag = Tag::new("news", Rect::new(0, 0, 120, 24));
        let plain = tag.size_hint().width;
        assert_eq!(tag.icon(), None, "no icon by default");

        tag.set_icon(Some(crate::widget::IconName::Check));
        assert_eq!(tag.icon(), Some(crate::widget::IconName::Check));
        assert_eq!(tag.size_hint().width, plain + TAG_ICON_SIDE + TAG_ICON_GAP);

        // The icon paints geometry, not a glyph, so the panel gains ink beyond the label.
        let svg = crate::widget::svg::render_to_svg(&mut tag);
        assert!(!svg.is_empty());

        tag.set_icon(None);
        assert_eq!(tag.size_hint().width, plain, "clearing the icon restores the old hint");
    }

    /// The icon is a writable property and the empty spelling clears it.
    #[test]
    fn the_icon_property_round_trips() {
        let mut tag = Tag::new("x", Rect::new(0, 0, 80, 24));
        tag.set("icon", CapabilityValue::String("check".to_string())).expect("writable");
        assert_eq!(tag.icon(), Some(crate::widget::IconName::Check));
        assert_eq!(tag.get("icon"), Ok(CapabilityValue::String("check".to_string())));
        tag.set("icon", CapabilityValue::String(String::new())).expect("clear");
        assert_eq!(tag.icon(), None);
        assert!(tag.set("icon", CapabilityValue::String("no-such-icon".to_string())).is_err());
    }

    /// The label face is constructed once per style, not once per frame.
    #[test]
    fn the_label_face_is_cached_while_the_style_font_is_unchanged() {
        let mut tag = Tag::new("cache", Rect::new(0, 0, 80, 24));
        let font = tag.resolved_font(None);
        let first = tag.font_cache.as_ref().expect("the face is cached").clone();
        assert_eq!(font, first);
        let _ = tag.resolved_font(None);
        assert_eq!(tag.font_cache.as_ref(), Some(&first), "a second frame reuses the cache");
    }

    /// The measured cell is cached until one of its four inputs changes.
    #[test]
    fn the_cell_measurement_is_cached_until_an_input_changes() {
        let tag = Tag::new("cache", Rect::new(0, 0, 80, 24));
        let first = tag.size_hint();
        let revision = tag.size_hint_revision();
        assert_eq!(tag.size_hint(), first, "a still tag reuses its measured cell");
        assert_eq!(tag.size_hint_revision(), revision, "a cache hit does not bump the revision");

        let mut changed = tag;
        changed.set_closable(true);
        assert!(changed.size_hint().width > first.width, "the close slot grows the cell");
        assert!(changed.size_hint_revision() > revision, "a rebuilt cell bumps the revision");
    }
}
