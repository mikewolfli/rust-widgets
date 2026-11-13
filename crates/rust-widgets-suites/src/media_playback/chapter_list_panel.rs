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
//! `WidgetKind::VideoPlayer`.

use crate::media_playback::video_player::{VideoChapter, VideoPlayer};
use rust_widgets::compat::{lock, Arc, Mutex, Vec};
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, GenericSignal, Signal1};
use rust_widgets::widget::capability::coercion::{
    expect_bool, expect_f64, expect_string, expect_u32,
};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{
    CapabilityAccessError, CapabilityValue, PropertyValueKind,
};
use rust_widgets::widget::capability::{
    RecordField, RecordList, RecordSchema, RecordValue, WidgetProperties,
};
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::view_widgets::rtl_columns::{is_rtl, leading_alignment, mirror_span};
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// RW-106: the chapter rows as records, so a design document can carry the list (the count alone
/// could not be written).
///
/// Chapter titles may repeat — a film can have two "Intro" chapters at different offsets — so the
/// schema allows duplicate and empty identities and uses the title as the identity only for
/// lookups.
pub const CHAPTER_RECORD_SCHEMA: RecordSchema = RecordSchema {
    id_field: "title",
    ids_unique: false,
    ids_required: false,
    fields: &[
        RecordField::required("title", PropertyValueKind::String),
        RecordField::required("start_seconds", PropertyValueKind::Float),
    ],
};

/// One chapter as a record, in the schema's field order.
fn chapter_record(chapter: &VideoChapter) -> RecordValue {
    RecordValue::new()
        .with("title", CapabilityValue::String(chapter.title.clone()))
        .with("start_seconds", CapabilityValue::Float(chapter.start_seconds))
}

/// The records a chapter list reports.
fn chapters_to_records(chapters: &[VideoChapter]) -> RecordList {
    RecordList::from_items(&CHAPTER_RECORD_SCHEMA, chapters.iter().map(chapter_record).collect())
}

/// Parses the chapter records, refusing a list that does not match the schema.
fn chapters_from_records(list: &RecordList) -> Result<Vec<VideoChapter>, CapabilityAccessError> {
    list.validate_against(&CHAPTER_RECORD_SCHEMA).map_err(|violation| {
        CapabilityAccessError::ConstraintViolation {
            property: rust_widgets::compat::String::from("chapters"),
            reason: rust_widgets::compat::format!("{violation}"),
        }
    })?;
    let mut chapters = Vec::with_capacity(list.items().len());
    for record in list.items() {
        let title = record
            .get("title")
            .cloned()
            .map(expect_string)
            .transpose()?
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let start_seconds = record
            .get("start_seconds")
            .cloned()
            .map(expect_f64)
            .transpose()?
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        chapters.push(VideoChapter::new(title, start_seconds));
    }
    Ok(chapters)
}

/// Width reserved for a chapter thumbnail when thumbnails are shown.
const CHAPTER_THUMBNAIL_WIDTH: u32 = 48;
/// Inset between a row's edges and its thumbnail.
const CHAPTER_THUMBNAIL_INSET: i32 = 4;
/// Horizontal padding inside a row.
const CHAPTER_PADDING: i32 = 8;
/// Width reserved for the start-time label at a row's trailing edge.
const CHAPTER_TIME_WIDTH: u32 = 56;

/// Resolves the surface, border and ink the panel paints with.
fn panel_palette(style: &rust_widgets::style::WidgetStyle, role: &str) -> (Color, Color, Color) {
    let theme = rust_widgets::style::resolved_theme_style_shared(role);
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
    /// The chapter start times the clock callback maps onto rows.
    ///
    /// Shared and rewritten by [`ChapterListPanel::set_chapters`] so the mapped index always
    /// names a row of the list on screen; a list captured once at attach time desynchronised as
    /// soon as an attached panel replaced its chapters.
    starts: Arc<Mutex<Vec<f64>>>,
    /// The followed clock position in milliseconds, when a player is attached.
    position: Arc<Mutex<Option<u64>>>,
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
/// is shared by painting and hit-testing through `Self::row_rect`.
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
    pub chapter_selected: Signal1<usize>,
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
            chapter_selected: Signal1::new(),
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
        if let Some(binding) = self.binding.as_ref() {
            *lock(&binding.starts) =
                self.chapters.iter().map(|chapter| chapter.start_seconds).collect();
        }
        self.rebuild_chapter_views();
    }

    /// Appends one chapter at its start-time position, returning that row index.
    ///
    /// A non-finite or negative start is refused (`None`), matching [`Self::set_chapters`]'s
    /// sanitising rule. The list stays sorted, so a row index means the same chapter here as in
    /// the player.
    pub fn add_chapter(&mut self, chapter: VideoChapter) -> Option<usize> {
        // While a player is bound it owns the chapter model (this companion owns the chrome), and
        // a row index must keep naming the same chapter on both sides. A local insert would make
        // the index this panel reports unresolvable in the player; the host pushes the edited list
        // through `VideoPlayer::set_chapters` instead.
        if self.is_attached() {
            return None;
        }
        if !chapter.start_seconds.is_finite() || chapter.start_seconds < 0.0 {
            return None;
        }
        let index = self
            .chapters
            .partition_point(|existing| existing.start_seconds <= chapter.start_seconds);
        self.chapters.insert(index, chapter);
        self.rebuild_chapter_views();
        Some(index)
    }

    /// Replaces the chapter at `index`, returning whether the index named a row.
    ///
    /// The list is re-sorted afterwards, because changing a start time moves the row; a
    /// non-finite or negative start is refused and leaves the list untouched.
    pub fn update_chapter(&mut self, index: usize, chapter: VideoChapter) -> bool {
        if self.is_attached() {
            return false;
        }
        if !chapter.start_seconds.is_finite() || chapter.start_seconds < 0.0 {
            return false;
        }
        let Some(slot) = self.chapters.get_mut(index) else {
            return false;
        };
        *slot = chapter;
        self.chapters.sort_by(|left, right| left.start_seconds.total_cmp(&right.start_seconds));
        self.rebuild_chapter_views();
        true
    }

    /// Removes the chapter at `index`, returning it when the index named a row.
    pub fn remove_chapter(&mut self, index: usize) -> Option<VideoChapter> {
        if self.is_attached() {
            return None;
        }
        if index >= self.chapters.len() {
            return None;
        }
        let chapter = self.chapters.remove(index);
        self.rebuild_chapter_views();
        Some(chapter)
    }

    /// Rebuilds the label column and re-derives the active row, scroll clamp and layout.
    fn rebuild_chapter_views(&mut self) {
        self.times =
            self.chapters.iter().map(|chapter| Self::format_start(chapter.start_seconds)).collect();
        // An attached panel's active row is the player's report, delivered through
        // `on_chapter_entered`. Re-deriving it from `position_ms` here would overwrite that
        // report with a clock the panel does not follow, so a list refresh would jump the
        // highlight back to the chapter under a stale position.
        let bound = self.binding.as_ref().map(|binding| *lock(&binding.active));
        let active = match bound {
            Some(Some(index)) if index < self.chapters.len() => Some(index),
            _ => self.chapter_index_at(self.position_ms),
        };
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
        match self.binding.as_ref().and_then(|binding| *lock(&binding.position)) {
            Some(position) => position,
            None => self.position_ms,
        }
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
        player.chapter_entered.connect_ref_scoped(&scope, move |index| {
            *lock(&active_sink) = Some(*index);
            redraw.emit();
        });
        // The chapter signal is only emitted on entry, so leaving every chapter (back before the
        // first marker) left the highlight naming a row the clock had left. The clock is the
        // player's published truth; map it with the panel's own rule.
        let starts: Arc<Mutex<Vec<f64>>> =
            Arc::new(Mutex::new(self.chapters.iter().map(|c| c.start_seconds).collect()));
        let clock_sink = Arc::clone(&active_mirror);
        let clock_redraw = self.base.redraw_requested_signal().clone();
        let clock_starts = Arc::clone(&starts);
        let clock_position = Arc::new(Mutex::new(Some(self.position_ms)));
        let position_sink = Arc::clone(&clock_position);
        player.time_updated.connect_ref_scoped(&scope, move |time| {
            // The property is read-only while attached, so the clock is its only source: publish
            // the followed position through the same cell the active index uses.
            *lock(&position_sink) = Some((time.max(0.0) * 1000.0) as u64);
            let starts = lock(&clock_starts);
            let next = starts.iter().rposition(|start| *start <= *time);
            let mut active = lock(&clock_sink);
            if *active != next {
                *active = next;
                clock_redraw.emit();
            }
        });

        let seeks = Arc::new(Mutex::new(Vec::new()));
        let seek_redraw = player.accept_chapter_seeks(Arc::clone(&seeks));
        self.seek_sink = Some(ChapterSeekSink { seeks, redraw: seek_redraw });
        self.binding = Some(ChapterBinding {
            active: active_mirror,
            starts,
            position: clock_position,
            _scope: scope,
        });
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
        // The painted row set is `0..visible_row_count()`, which can be empty when the panel is
        // shorter than one row; `row_rect` alone accepts any non-empty band, so it answered for a
        // row the paint never draws and a press there selected a chapter that was not on screen.
        if visible >= self.visible_row_count() {
            return None;
        }
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
        self.chapter_selected.emit(index);
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
    fn capability_name(&self) -> Option<&'static str> {
        Some("chapter_list_panel")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        rust_widgets::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            rust_widgets::style::EdgeOffsets::all(0),
            Size::new(220, 180),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "chapter_selected" => {
                Some(EventSignalRef::mapped("chapter_selected", &self.chapter_selected, |index| {
                    CapabilityValue::UInt(*index as u64)
                }))
            }
            // Legacy spelling (DX-10): the name published before the `on_` prefix was removed.
            "on_chapter_selected" => Some(EventSignalRef::mapped(
                "on_chapter_selected",
                &self.chapter_selected,
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
            // RW-106: the list itself, and the count under its own name. `chapters` used to be
            // the count, which left no writable carrier for the rows.
            "chapters" => Ok(CapabilityValue::Records(chapters_to_records(&self.chapters))),
            "chapter_count" => Ok(CapabilityValue::UInt(self.chapters.len() as u64)),
            // The property follows the attached clock through the accessor; reading the field
            // froze the readout at the value captured when the player was bound.
            "position_ms" => Ok(CapabilityValue::UInt(self.position_ms())),
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
            // RW-106: replaces the list through the control's own setter, so layout, selection
            // and the attached player's seek queue all follow the same path a Rust caller uses.
            "chapters" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let chapters = chapters_from_records(&list)?;
                self.set_chapters(chapters);
                Ok(())
            }
            "chapter_count" | "position_ms" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "row_height",
            "active_highlight",
            "show_thumbnails",
            "chapters",
            "chapter_count",
            "position_ms",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `chapter_list_panel` publishes.
    ///
    /// `set_chapters` carries its value through the property route, so a bare invocation is
    /// [`CapabilityAccessError::OutOfRange`] and the caller writes the property instead.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        self.default_command(name)
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
        // Direction is read once for the frame. A row is the band every item in it is reflected
        // in, so the thumbnail, the title and the start time swap ends together and keep their
        // widths; the row itself is full-width, so nothing else about the geometry changes.
        let rtl = is_rtl();
        let leading = leading_alignment();

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
                let thumb = if rtl {
                    Rect::new(
                        mirror_span(thumb.x, thumb.width, row),
                        thumb.y,
                        thumb.width,
                        thumb.height,
                    )
                } else {
                    thumb
                };
                context.fill_rect(thumb, Color::rgba(0, 0, 0, 24));
                context.draw_rect(thumb, border);
                text_x += CHAPTER_THUMBNAIL_WIDTH as i32 + CHAPTER_PADDING;
            }

            let time_x = row.x + row.width as i32 - CHAPTER_TIME_WIDTH as i32 - CHAPTER_PADDING;
            let title_width = (time_x - text_x - CHAPTER_PADDING).max(0) as u32;
            let title_x = if rtl { mirror_span(text_x, title_width, row) } else { text_x };
            let title = Rect::new(title_x, row.y, title_width, row.height);
            context.draw_text_line_ellipsis(title, &chapter.title, font, ink, leading);
            let time = self.times.get(index).map(String::as_str).unwrap_or("");
            let time_x = if rtl { mirror_span(time_x, CHAPTER_TIME_WIDTH, row) } else { time_x };
            let time_band = Rect::new(time_x, row.y, CHAPTER_TIME_WIDTH, row.height);
            // The start time sits on the row's trailing edge, which is the left one in RTL.
            let time_alignment =
                if rtl { HorizontalAlignment::Left } else { HorizontalAlignment::Right };
            context.draw_text_line_ellipsis(time_band, time, font, ink, time_alignment);
            context.draw_line(
                Point::new(row.x, row.bottom()),
                Point::new(row.x + row.width as i32, row.bottom()),
                border,
            );
        }
        // Rows answer the pointer through selection, but the panel had no hover state at all;
        // the shared wash is the control-level cue (W5-04).
        self.base.paint_hover_wash(context, rect);
        crate::veil_if_disabled(&self.base, context);
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
        panel.chapter_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        panel.handle_event(&Event::MousePress { pos: Point::new(20, 35), button: 1, modifiers: 0 });

        assert_eq!(panel.active_chapter(), Some(1));
        assert_eq!(seen.lock().map(|guard| guard.clone()).unwrap_or_default(), alloc::vec![1usize]);
    }

    /// A panel shorter than one row paints nothing, so it has no hit target either.
    ///
    /// `hit_chapter` validated the row against `row_rect`, which accepts any non-empty band, so a
    /// 20 px panel below its 32 px row height reported row 0 and a press selected a chapter that
    /// was not on screen.
    #[test]
    fn a_panel_shorter_than_one_row_has_no_hit_target() {
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 20));
        panel.set_row_height(32);
        panel.set_chapters(chapters());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        panel.chapter_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        assert_eq!(panel.hit_chapter(Point::new(20, 5)), None);
        let before = panel.active_chapter();
        panel.handle_event(&Event::MousePress { pos: Point::new(20, 5), button: 1, modifiers: 0 });
        assert_eq!(panel.active_chapter(), before, "a press on nothing selects nothing");
        assert!(seen.lock().map(|guard| guard.clone()).unwrap_or_default().is_empty());
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

    /// RW-87: chapters can be edited one row at a time and stay in start order.
    #[test]
    fn chapters_can_be_added_updated_and_removed_incrementally() {
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.set_chapters(chapters());

        let index =
            panel.add_chapter(VideoChapter::new("Late", 90.0)).expect("a finite start is accepted");
        assert_eq!(index, 2, "90s sits between 60s and 120s");
        assert_eq!(panel.chapters()[2].title, "Late");
        assert_eq!(panel.add_chapter(VideoChapter::new("Bad", f64::NAN)), None);
        assert_eq!(panel.add_chapter(VideoChapter::new("Negative", -1.0)), None);

        assert!(panel.update_chapter(2, VideoChapter::new("Later", 130.0)));
        assert_eq!(panel.chapters().len(), 4);
        assert_eq!(panel.chapters()[3].title, "Later", "the edited row is re-sorted");
        assert!(!panel.update_chapter(9, VideoChapter::new("X", 1.0)));

        let removed = panel.remove_chapter(3).expect("the row exists");
        assert_eq!(removed.title, "Later");
        assert_eq!(panel.chapters().len(), 3);
        assert!(panel.remove_chapter(9).is_none());
    }

    /// Refreshing the list must not overwrite the player's live chapter report with the
    /// panel's own (unfollowed) position.
    #[test]
    fn refreshing_the_list_keeps_the_players_active_chapter() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);

        player.seek(65.0);
        assert_eq!(panel.active_chapter(), Some(1), "the player reports the chapter it entered");

        let rows = panel.chapters().to_vec();
        panel.set_chapters(rows);
        assert_eq!(panel.active_chapter(), Some(1), "a refresh must keep the player's report");
        assert_eq!(player.current_chapter(), Some(1));
    }

    /// Leaving every chapter clears the highlight an attached player reported.
    #[test]
    fn leaving_every_chapter_clears_the_attached_highlight() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(vec![VideoChapter::new("Intro", 10.0)]);
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);

        player.seek(15.0);
        assert_eq!(panel.active_chapter(), Some(0));
        player.seek(0.0);
        assert_eq!(player.current_chapter(), None);
        assert_eq!(
            panel.active_chapter(),
            None,
            "the panel must follow the player out of the chapter"
        );
    }

    /// An attached panel refuses local row edits: the player owns the row set.
    #[test]
    fn an_attached_panel_refuses_local_row_edits() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);

        assert_eq!(panel.add_chapter(VideoChapter::new("Late", 90.0)), None);
        assert!(!panel.update_chapter(0, VideoChapter::new("Changed", 5.0)));
        assert_eq!(panel.remove_chapter(1), None);
        assert_eq!(panel.chapters().len(), 3, "the player's rows are untouched");
    }

    /// Replacing the rows while attached rewrites the clock mapping with them.
    ///
    /// The start list was captured once at attach; a replacement left the clock mapping onto the
    /// old starts and could name a row that no longer exists.
    #[test]
    fn replacing_chapters_while_attached_keeps_the_mapping_in_step() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());
        player.seek(65.0);
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);
        assert_eq!(panel.active_chapter(), Some(1));

        panel.set_chapters(alloc::vec![VideoChapter::new("A", 0.0), VideoChapter::new("B", 90.0),]);
        player.seek(95.0);
        assert_eq!(panel.active_chapter(), Some(1));
        assert!(panel.active_chapter().expect("a row") < panel.chapters().len());
    }

    /// While attached, `position_ms` follows the player's clock instead of freezing at bind time.
    #[test]
    fn position_ms_follows_the_attached_clock() {
        use rust_widgets::widget::capability::WidgetProperties;

        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(200.0);
        player.set_chapters(chapters());
        let mut panel = ChapterListPanel::new(Rect::new(0, 0, 240, 120));
        panel.attach_video_player(&mut player);
        assert_eq!(panel.position_ms(), 0);

        player.seek(65.0);
        assert_eq!(panel.position_ms(), 65_000, "the readout follows the followed clock");
        assert_eq!(
            panel.get("position_ms"),
            Ok(CapabilityValue::UInt(65_000)),
            "the published property reads the same accessor, not the frozen field"
        );
    }

    /// The ink boxes one draw leaves, sorted so two draws can be compared position by position.
    fn ink_boxes(widget: &mut impl Draw, geometry: Rect) -> Vec<(i32, i32, i32, i32)> {
        let svg = rust_widgets::widget::svg::render_widget_to_svg(widget, geometry);
        let mut boxes = rust_widgets::widget::svg::text_ink_boxes(&svg);
        boxes.sort_unstable();
        boxes
    }

    /// An RTL page reflects the thumbnail, title and start-time columns about the row band
    /// (W5-15).
    ///
    /// The row is full-width, so the reflection is the row's own mirror: the thumbnail moves to
    /// the right, the start time to the left, and the title takes the band between them.
    #[test]
    fn rtl_mirrors_the_row_columns() {
        let geometry = Rect::new(0, 0, 240, 60);
        let mut ltr_panel = ChapterListPanel::new(geometry);
        ltr_panel.set_row_height(30);
        ltr_panel.set_show_thumbnails(true);
        ltr_panel.set_chapters(chapters());
        let ltr = ink_boxes(&mut ltr_panel, geometry);
        assert_eq!(ltr.len(), 4, "two visible rows, each a title and a start time");

        let previous = rust_widgets::core::default_text_direction();
        rust_widgets::core::set_default_text_direction(
            rust_widgets::core::TextDirection::RightToLeft,
        );
        let mut rtl_panel = ChapterListPanel::new(geometry);
        rtl_panel.set_row_height(30);
        rtl_panel.set_show_thumbnails(true);
        rtl_panel.set_chapters(chapters());
        let rtl = ink_boxes(&mut rtl_panel, geometry);
        rust_widgets::core::set_default_text_direction(previous);

        assert_eq!(rtl.len(), ltr.len(), "the same runs are drawn in both directions");
        let edge = geometry.x + geometry.width as i32;
        let mut mirrored: Vec<(i32, i32, i32, i32)> = ltr
            .iter()
            .map(|&(left, top, right, bottom)| (edge - right, top, edge - left, bottom))
            .collect();
        mirrored.sort_unstable();
        for (expected, actual) in mirrored.iter().zip(rtl.iter()) {
            // The run's *placement* is an exact mirror, but its ink starts at the first glyph's
            // left bearing and ends at the last glyph's right bearing, so the two edges of one
            // run can differ by a few pixels once the run is placed on the other side.
            assert!(
                (expected.0 - actual.0).abs() <= 4 && (expected.2 - actual.2).abs() <= 4,
                "run {actual:?} is not the mirror of {expected:?}"
            );
        }
    }
}
