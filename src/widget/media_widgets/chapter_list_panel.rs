// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ChapterListPanel — chapter navigation beside or over a [`VideoPlayer`] (COMP-A6).
//!
//! The player owns the chapter model and reports the chapter the clock enters; this companion
//! owns the list chrome. Rows are the player's chapters in start order, each showing a title and
//! its formatted start time. A press selects a row, emits its index, and — when a player is
//! attached — queues that chapter's start time in the player's seek stream.
//!
//! The panel cannot hold a borrow of the player, so the pairing is two shared channels: the
//! player's `on_chapter_entered` signal pushes the active index here, and a shared seek queue
//! (the same `ColorWell`/`ColorHistory` pattern) pushes a press back to the player.
//!
//! Registration surface (out of scope for this file): canonical name `chapter_list_panel`, kind
//! [`WidgetKind::VideoPlayer`].

use crate::compat::{lock, Arc, Mutex, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::media_widgets::video_player::{VideoChapter, VideoPlayer};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Width reserved for a chapter thumbnail when thumbnails are shown.
const CHAPTER_THUMBNAIL_WIDTH: u32 = 48;
/// Inset between a row's edges and its thumbnail.
const CHAPTER_THUMBNAIL_INSET: i32 = 4;
/// Horizontal padding inside a row.
const CHAPTER_PADDING: i32 = 8;
/// Width reserved for the start-time label at a row's trailing edge.
const CHAPTER_TIME_WIDTH: u32 = 56;

/// Resolves the surface, border and ink the panel paints with.
fn panel_palette(style: &crate::style::WidgetStyle, role: &str) -> (Color, Color, Color) {
    let theme = crate::style::resolved_theme_style_shared(role);
    let ink = style
        .text_color
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or(Color::rgb(45, 58, 74));
    let border = style
        .border_color
        .or_else(|| theme.as_ref().and_then(|t| t.border_color))
        .unwrap_or(Color::rgb(168, 184, 201));
    let surface = style
        .background_color
        .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        .unwrap_or(Color::rgb(235, 243, 250));
    (surface, border, ink)
}

/// The live chapter index an attached player reports.
struct ChapterBinding {
    active: Arc<Mutex<Option<usize>>>,
    _scope: ConnectionScope,
}

/// The write end of a player's chapter-seek stream.
struct ChapterSeekSink {
    seeks: Arc<Mutex<Vec<f64>>>,
    redraw: GenericSignal,
}

/// A vertical list of a video's chapters.
///
/// The panel mirrors the player's sanitising rule — non-finite and negative starts are dropped
/// and the rest are sorted — so a row index means the same chapter in both controls. Row geometry
/// is shared by painting and hit-testing through [`Self::row_rect`].
pub struct ChapterListPanel {
    base: BaseWidget,
    chapters: Vec<VideoChapter>,
    /// Formatted start labels, rebuilt with the chapter list rather than per frame.
    times: Vec<String>,
    active: Option<usize>,
    position_ms: u64,
    active_highlight: bool,
    show_thumbnails: bool,
    row_height: u32,
    scroll_offset: usize,
    binding: Option<ChapterBinding>,
    seek_sink: Option<ChapterSeekSink>,
    /// Emitted with the chapter index a press selected.
    pub on_chapter_selected: Signal1<usize>,
}

impl ChapterListPanel {
    /// Creates an empty panel with a 32 px row height and active highlighting on.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VideoPlayer, geometry, "ChapterListPanel"),
            chapters: Vec::new(),
            times: Vec::new(),
            active: None,
            position_ms: 0,
            active_highlight: true,
            show_thumbnails: false,
            row_height: 32,
            scroll_offset: 0,
            binding: None,
            seek_sink: None,
            on_chapter_selected: Signal1::new(),
        }
    }

    /// Replaces the chapter list, applying the player's sanitising rule.
    ///
    /// The active index is recomputed from the current position, so a list installed while the
    /// clock is already inside a chapter highlights that chapter immediately.
    pub fn set_chapters(&mut self, chapters: Vec<VideoChapter>) {
        let mut chapters: Vec<VideoChapter> = chapters
            .into_iter()
            .filter(|chapter| chapter.start_seconds.is_finite() && chapter.start_seconds >= 0.0)
            .collect();
        chapters.sort_by(|left, right| left.start_seconds.total_cmp(&right.start_seconds));
        self.chapters = chapters;
        self.times =
            self.chapters.iter().map(|chapter| Self::format_start(chapter.start_seconds)).collect();
        let active = self.chapter_index_at(self.position_ms);
        self.set_active(active);
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The chapter markers currently listed.
    pub fn chapters(&self) -> &[VideoChapter] {
        &self.chapters
    }

    /// Whether the active row is highlighted.
    pub fn active_highlight(&self) -> bool {
        self.active_highlight
    }

    /// Enables or disables the active-row highlight.
    pub fn set_active_highlight(&mut self, active_highlight: bool) {
        if self.active_highlight == active_highlight {
            return;
        }
        self.active_highlight = active_highlight;
        self.base.request_redraw();
    }

    /// Whether each row reserves a thumbnail band.
    pub fn show_thumbnails(&self) -> bool {
        self.show_thumbnails
    }

    /// Shows or hides the per-row thumbnail band.
    ///
    /// No thumbnail provider exists in the player model, so the band is a framed placeholder;
    /// a host that has frames can paint into the same reserved geometry.
    pub fn set_show_thumbnails(&mut self, show_thumbnails: bool) {
        if self.show_thumbnails == show_thumbnails {
            return;
        }
        self.show_thumbnails = show_thumbnails;
        self.base.request_redraw();
    }

    /// The row height in pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the row height, floored at 8 pixels.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(8);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The playback position the panel maps to a chapter, in milliseconds.
    pub fn position_ms(&self) -> u64 {
        self.position_ms
    }

    /// Sets the playback position and moves the active chapter to the one containing it.
    pub fn set_position_ms(&mut self, position_ms: u64) {
        self.position_ms = position_ms;
        let active = self.chapter_index_at(position_ms);
        if self.active_chapter() != active {
            self.set_active(active);
            self.base.request_redraw();
        }
    }

    /// The chapter containing the current position, following an attached player's reports.
    pub fn active_chapter(&self) -> Option<usize> {
        match &self.binding {
            Some(binding) => *lock(&binding.active),
            None => self.active,
        }
    }

    /// Binds this panel to `player`: adopts its chapters and clock, then follows the chapter the
    /// player enters and queues a seek when a row is pressed.
    ///
    /// Calling this again replaces the previous subscriptions and seek stream.
    pub fn attach_video_player(&mut self, player: &mut VideoPlayer) {
        self.set_chapters(player.chapters().to_vec());
        self.position_ms = (player.current_time().max(0.0) * 1000.0) as u64;
        let active = player.current_chapter().or_else(|| self.chapter_index_at(self.position_ms));
        self.active = active;

        let active_mirror = Arc::new(Mutex::new(active));
        let scope = ConnectionScope::new();
        let active_sink = Arc::clone(&active_mirror);
        let redraw = self.base.redraw_requested_signal().clone();
        player.on_chapter_entered.connect_ref_scoped(&scope, move |index| {
            *lock(&active_sink) = Some(*index);
            redraw.emit();
        });

        let seeks = Arc::new(Mutex::new(Vec::new()));
        let seek_redraw = player.accept_chapter_seeks(Arc::clone(&seeks));
        self.seek_sink = Some(ChapterSeekSink { seeks, redraw: seek_redraw });
        self.binding = Some(ChapterBinding { active: active_mirror, _scope: scope });
        self.base.request_redraw();
    }

    /// Whether a video player is attached.
    pub fn is_attached(&self) -> bool {
        self.binding.is_some()
    }

    /// Scrolls so the active chapter is on screen. Returns whether the list moved.
    pub fn scroll_to_active(&mut self) -> bool {
        let Some(index) = self.active_chapter().or_else(|| self.chapter_index_at(self.position_ms))
        else {
            return false;
        };
        if index >= self.chapters.len() {
            return false;
        }
        let rows = self.visible_row_count().max(1);
        let next = if index < self.scroll_offset {
            index
        } else if index >= self.scroll_offset + rows {
            index + 1 - rows
        } else {
            self.scroll_offset
        };
        self.scroll_to(next)
    }

    /// The chapter index under `point`, using the geometry [`Draw`] paints.
    pub fn hit_chapter(&self, point: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(point) {
            return None;
        }
        let local_y = point.y - rect.y;
        if local_y < 0 {
            return None;
        }
        let visible = (local_y / self.row_height as i32) as usize;
        self.row_rect(visible)?;
        let index = self.scroll_offset + visible;
        (index < self.chapters.len()).then_some(index)
    }

    /// The first on-screen row, in rows scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the list by `delta` rows, clamped to the chapter range.
    pub fn scroll_by(&mut self, delta: isize) {
        let current = self.scroll_offset;
        let next = (current as isize + delta).clamp(0, self.max_scroll() as isize) as usize;
        self.scroll_to(next);
    }

    /// Sets the active index, mirroring it into the attached player binding.
    fn set_active(&mut self, active: Option<usize>) {
        self.active = active;
        if let Some(binding) = &self.binding {
            *lock(&binding.active) = active;
        }
    }

    /// The last chapter whose start is at or before `position_ms`.
    fn chapter_index_at(&self, position_ms: u64) -> Option<usize> {
        self.chapters
            .iter()
            .rposition(|chapter| (chapter.start_seconds.max(0.0) * 1000.0) <= position_ms as f64)
    }

    /// Formats a start time as `MM:SS`, or `H:MM:SS` past an hour.
    fn format_start(seconds: f64) -> String {
        let total = seconds.max(0.0).round() as u64;
        let hours = total / 3600;
        let minutes = (total % 3600) / 60;
        let secs = total % 60;
        if hours > 0 {
            format!("{hours}:{minutes:02}:{secs:02}")
        } else {
            format!("{minutes:02}:{secs:02}")
        }
    }

    /// How many rows fit in the control.
    fn visible_row_count(&self) -> usize {
        (self.geometry().height / self.row_height) as usize
    }

    /// The largest valid scroll offset.
    fn max_scroll(&self) -> usize {
        self.chapters.len().saturating_sub(self.visible_row_count())
    }

    /// The geometry of one on-screen row, shared by painting and hit-testing.
    fn row_rect(&self, visible: usize) -> Option<Rect> {
        let rect = self.geometry();
        let top = rect.y + visible as i32 * self.row_height as i32;
        let bottom = rect.y + rect.height as i32;
        if top >= bottom {
            return None;
        }
        let height = (self.row_height as i32).min(bottom - top) as u32;
        Some(Rect::new(rect.x, top, rect.width, height))
    }

    /// Moves to an absolute row offset, clamped. Returns whether it moved.
    fn scroll_to(&mut self, offset: usize) -> bool {
        let next = offset.min(self.max_scroll());
        if next == self.scroll_offset {
            return false;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
        true
    }

    /// Re-clamps the stored offset after the row set or geometry changed.
    fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
    }

    /// Selects a row: positions the panel, reports the index, and queues a player seek.
    fn select_chapter(&mut self, index: usize) -> bool {
        let Some(chapter) = self.chapters.get(index) else {
            return false;
        };
        let start = chapter.start_seconds.max(0.0);
        self.position_ms = (start * 1000.0) as u64;
        // The pressed row is the selection even when two chapters share a start time, where
        // "the chapter containing this instant" would pick the later one.
        self.set_active(Some(index));
        self.on_chapter_selected.emit(index);
        if let Some(sink) = &self.seek_sink {
            lock(&sink.seeks).push(start);
            // Wake the player, so its paint drains the queue and moves the clock.
            sink.redraw.emit();
        }
        self.base.request_redraw();
        true
    }
}

impl Widget for ChapterListPanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        Size::new(220, 180)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "chapter_selected" | "on_chapter_selected" => Some(EventSignalRef::mapped(
                "chapter_selected",
                &self.on_chapter_selected,
                |index| CapabilityValue::UInt(*index as u64),
            )),
            _ => None,
        }
    }
}

impl WidgetProperties for ChapterListPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height))),
            "active_highlight" => Ok(CapabilityValue::Bool(self.active_highlight)),
            "show_thumbnails" => Ok(CapabilityValue::Bool(self.show_thumbnails)),
            "chapters" => Ok(CapabilityValue::UInt(self.chapters.len() as u64)),
            "position_ms" => Ok(CapabilityValue::UInt(self.position_ms)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "active_highlight" => {
                self.set_active_highlight(expect_bool(value)?);
                Ok(())
            }
            "show_thumbnails" => {
                self.set_show_thumbnails(expect_bool(value)?);
                Ok(())
            }
            "chapters" | "position_ms" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "row_height",
            "active_highlight",
            "show_thumbnails",
            "chapters",
            "position_ms",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for ChapterListPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.hit_chapter(*pos) {
                    self.select_chapter(index);
                }
            }
            Event::Wheel { delta, .. } if delta.y != 0 => {
                self.scroll_by(if delta.y > 0 { -1 } else { 1 });
            }
            _ => {}
        }
    }
}

impl Draw for ChapterListPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let (surface, border, ink) = panel_palette(style, "chapter_list_panel");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let font = effective_font(style);
        let active = self.active_chapter();

        for visible in 0..self.visible_row_count() {
            let index = self.scroll_offset + visible;
            let Some(chapter) = self.chapters.get(index) else {
                break;
            };
            let Some(row) = self.row_rect(visible) else {
                break;
            };
            if self.active_highlight && active == Some(index) {
                context.fill_rect(row, Color::rgba(80, 130, 190, 48));
            }

            let mut text_x = row.x + CHAPTER_PADDING;
            if self.show_thumbnails {
                let thumb_height = (row.height as i32 - 2 * CHAPTER_THUMBNAIL_INSET).max(1) as u32;
                let thumb = Rect::new(
                    text_x,
                    row.y + CHAPTER_THUMBNAIL_INSET,
                    CHAPTER_THUMBNAIL_WIDTH,
                    thumb_height,
                );
                context.fill_rect(thumb, Color::rgba(0, 0, 0, 24));
                context.draw_rect(thumb, border);
                text_x += CHAPTER_THUMBNAIL_WIDTH as i32 + CHAPTER_PADDING;
            }

            let time_x = row.x + row.width as i32 - CHAPTER_TIME_WIDTH as i32 - CHAPTER_PADDING;
            let title_width = (time_x - text_x - CHAPTER_PADDING).max(0) as u32;
            let title = Rect::new(text_x, row.y, title_width, row.height);
            context.draw_text_line_cow(title, &chapter.title, font, ink, HorizontalAlignment::Left);
            let time = self.times.get(index).map(String::as_str).unwrap_or("");
            let time_band = Rect::new(time_x, row.y, CHAPTER_TIME_WIDTH, row.height);
            context.draw_text_line_cow(time_band, time, font, ink, HorizontalAlignment::Right);
            context.draw_line(
                Point::new(row.x, row.bottom()),
                Point::new(row.x + row.width as i32, row.bottom()),
                border,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapters() -> Vec<VideoChapter> {
        alloc::vec![
            VideoChapter::new("Intro", 0.0),
            VideoChapter::new("Middle", 60.0),
            VideoChapter::new("Finale", 120.0),
        ]
    }

    /// A press maps to the row under the pointer and reports its index.
    #[test]
    fn a_press_maps_to_the_chapter_row() {
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.set_row_height(30);
        panel.set_chapters(chapters());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        panel.on_chapter_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        panel.handle_event(&Event::MousePress { pos: Point::new(20, 35), button: 1, modifiers: 0 });

        assert_eq!(panel.active_chapter(), Some(1));
        assert_eq!(seen.lock().map(|guard| guard.clone()).unwrap_or_default(), alloc::vec![1usize]);
    }

    /// An attached panel follows the player's chapter-entry reports.
    #[test]
    fn attach_follows_the_players_chapter_reports() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());
        player.seek(1.0);
        assert_eq!(player.current_chapter(), Some(0));

        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);
        assert_eq!(panel.chapters().len(), 3);
        assert_eq!(panel.active_chapter(), Some(0));

        player.seek(65.0);
        assert_eq!(player.current_chapter(), Some(1));
        assert_eq!(panel.active_chapter(), Some(1), "the chapter-entered signal reaches the list");
    }

    /// A press on an attached panel queues a seek that the player applies.
    #[test]
    fn a_press_on_an_attached_panel_seeks_the_player() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());

        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.set_row_height(30);
        panel.attach_video_player(&mut player);

        panel.handle_event(&Event::MousePress { pos: Point::new(20, 65), button: 1, modifiers: 0 });
        assert_eq!(panel.active_chapter(), Some(2));
        assert_eq!(player.apply_pending_chapter_seeks(), 1);
        assert!((player.current_time() - 120.0).abs() < f64::EPSILON);
    }

    /// `set_position_ms` maps the clock onto the chapter that contains it.
    #[test]
    fn position_maps_to_the_containing_chapter() {
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.set_chapters(chapters());
        panel.set_position_ms(59_000);
        assert_eq!(panel.active_chapter(), Some(0));
        panel.set_position_ms(60_000);
        assert_eq!(panel.active_chapter(), Some(1));
        panel.set_position_ms(10_000);
        assert_eq!(panel.active_chapter(), Some(0));
    }

    /// Stored offsets are clamped when a shorter list is installed.
    #[test]
    fn a_shorter_list_clamps_the_scroll() {
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 60));
        panel.set_row_height(30);
        panel.set_chapters(alloc::vec![
            VideoChapter::new("One", 0.0),
            VideoChapter::new("Two", 10.0),
            VideoChapter::new("Three", 20.0),
            VideoChapter::new("Four", 30.0),
        ]);
        panel.scroll_by(10);
        assert_eq!(panel.scroll_offset(), 2);
        panel.set_chapters(chapters());
        assert_eq!(panel.scroll_offset(), 1, "three rows leave one scroll step");
    }
}
