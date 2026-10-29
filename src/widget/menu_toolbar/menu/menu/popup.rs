// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The menu popup: row layout, nested-panel painting and pointer hit testing.
//!
//! Everything here is a pure function of the model in `model`: the row bands, the
//! derived indicator/label/trailing columns and the panel rectangles. The entry list
//! itself is owned by the model, so the painter and the hit paths read one state.

use crate::compat::{Rc, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::menu_toolbar::effects;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width};
use crate::widget::{Draw, IconName, Widget};

use super::model::{
    strip_menu_mnemonic, ItemBandsKey, Menu, MenuEntry, MnemonicRow, PopupDirection,
    PopupMetricsKey, MENU_COLUMN_GAP,
};

impl Menu {
    /// Opens this menu as a context menu at `position`.
    ///
    /// `viewport` is the area the popup must stay inside — the widget's parent
    /// surface. The popup is shifted left/up when it would overflow, which is what
    /// keeps a menu opened near the right or bottom edge fully reachable instead of
    /// clipped at the edge.
    ///
    /// The popup's vertical anchor follows [`Self::popup_direction`]. With the default
    /// [`PopupDirection::Auto`] the popup opens downward when it fits and is anchored by its
    /// bottom edge when the remaining space below the invocation point is too small, so a
    /// menu opened near the bottom of the screen flips up instead of being clipped.
    pub fn open_at(&mut self, position: Point, viewport: Rect) {
        // A menu created through `create_menu(.., width = 0, height = 0)` — the shape every
        // documented call site uses — has a zero-width geometry. Opening it as-is painted a
        // popup with no pixels at all, so the click that asked for it looked like a no-op.
        // The popup takes the width it was given when that is usable, and the control's own
        // size hint otherwise.
        let rect = self.geometry();
        let popup_w = rect.width.max(self.size_hint().width).max(1) as i32;
        // The drawn popup is its heading strip **plus** the body; both are part of what must
        // fit inside the viewport. Measuring only the body let the heading hang off the top
        // edge when a menu was opened near it.
        let popup_h = self.drawn_heading_height() as i32 + self.popup_height() as i32;

        // Clamp so the whole popup fits; never go negative, because a popup at a
        // negative coordinate is off-screen on every backend.
        let max_x = (viewport.x + viewport.width as i32).saturating_sub(popup_w);
        let max_y = (viewport.y + viewport.height as i32).saturating_sub(popup_h);
        let x = position.x.clamp(viewport.x, max_x.max(viewport.x));
        let below = viewport.y + viewport.height as i32 - position.y;
        let opens_up = match self.popup_direction {
            PopupDirection::Down => false,
            PopupDirection::Up => true,
            // Flipping only when the space above actually fits avoids trading a clipped
            // bottom for a clipped top on a viewport shorter than the popup itself.
            PopupDirection::Auto => {
                below < popup_h && position.y.saturating_sub(popup_h) >= viewport.y
            }
        };
        let anchor_y = if opens_up { position.y.saturating_sub(popup_h) } else { position.y };
        let y = anchor_y.clamp(viewport.y, max_y.max(viewport.y));

        // The geometry is the rectangle `contains_point`, the hit test and `render_frame`
        // read, so it must be the rectangle `draw` paints rather than the caller-supplied
        // 0x0 box.
        self.set_geometry(Rect::new(x, y, popup_w as u32, popup_h.max(1) as u32));
        self.invoker_position = Some(position);
        // A nested panel is placed against the same surface bound; storing it here is
        // what lets `submenu_panel_rect_at` clamp and flip before the panel is popped.
        self.viewport = Some(viewport);
        // A newly opened popup starts at the top of its list; the previous open may have
        // left it scrolled.
        self.scroll_offset = 0;
        self.clamp_scroll();
        // A new open starts at the top level: a cascade left over from the previous open
        // would describe a branch of a menu the user is no longer looking at.
        self.submenu_path.clear();
        // Pre-select the first actionable entry so keyboard navigation has a
        // starting point; a menu opened with no selection cannot be driven by
        // arrow keys without a first press that has no visible effect.
        self.hovered_index =
            self.items.iter().position(|item| !item.is_separator() && item.is_enabled());
        self.show();
        // A menu is a **command surface**, not a decoration: it must be on screen the moment the
        // user asks for it. The reveal still animates on close, but an opening popup that grew
        // over ~30 frames was reported as "菜单加载好慢" — and macOS shows a menu immediately.
        // `jump_to` also guarantees the first painted frame contains every row, so a click that
        // follows the opening press lands on the row the user can see.
        self.reveal.jump_to(true);
    }

    /// The popup body's top edge: the geometry's `y` plus the heading strip.
    ///
    /// `draw` paints the heading first and lays the entry rows out below it. The pointer
    /// handlers used to start their row walk at `geometry().y`, so every row was read as
    /// `heading_height` pixels higher than it was drawn — a click on the lower half of one
    /// row activated the row below it. One derivation, used by the painter and by the hit
    /// paths, is what keeps them from drifting apart again.
    pub(super) fn body_top(&self) -> i32 {
        self.geometry().y + self.drawn_heading_height() as i32
    }

    /// Whether `point` lies inside the popup's drawn area or one of its open panels.
    ///
    /// The drawn height follows the item list, not `geometry().height`, because a
    /// menu is as tall as its entries — using the widget's stored height would make
    /// the "outside" test disagree with what the user sees. A nested panel hangs to the
    /// right of the popup, outside that rectangle, so it is part of the accepted area too:
    /// a press on a child row must reach this control rather than the widget behind it.
    pub fn contains_point(&self, point: Point) -> bool {
        let rect = self.geometry();
        // The drawn extent is the heading **plus** the body. Testing only `popup_height`
        // from `rect.y` both accepted clicks on the heading and rejected clicks on the
        // body's lower rows once the popup was taller than the heading.
        let inside_popup = point.x >= rect.x
            && point.x < rect.x + rect.width as i32
            && point.y >= rect.y
            && point.y < self.body_top() + self.popup_height() as i32;
        inside_popup
            || (1..=self.submenu_path.len())
                .filter_map(|level| self.submenu_panel_rect_at(level))
                .any(|panel| {
                    point.x >= panel.x
                        && point.x < panel.x + panel.width as i32
                        && point.y >= panel.y
                        && point.y < panel.y + panel.height as i32
                })
    }
    pub(super) fn item_height() -> f32 {
        22.0
    }

    /// The box of every entry, separators included, in popup coordinates.
    ///
    /// The boxes are in **popup coordinates** — the origin is the popup's top-left, not the
    /// control's — because that is the frame the entries are painted in; the caller translates to
    /// `rect` when it draws.
    ///
    /// The popup's own `MENU_POPUP_PADDING` is the column's padding rather than a `+ 2.0` the paint
    /// loop added before the first row, so the inset is part of the run instead of a constant that
    /// only the first row knows about.
    /// The row boxes, served from a cache keyed by the list version and geometry.
    ///
    /// The run is a pure function of the entries' separator flags, the popup's width, the scroll
    /// offset, the tear-off strip and the column count. Rebuilding it on every paint meant a
    /// `Vec` allocation per frame during the open reveal even though none of those facts had
    /// changed; the version counter is bumped by every entry mutation, so a stale run cannot be
    /// served after an edit.
    pub(super) fn item_bands(&self) -> Rc<Vec<Rect>> {
        let key = ItemBandsKey {
            version: self.items_version,
            width: self.geometry().width,
            scroll_offset: self.scroll_offset,
            tearoff: self.tearoff,
            columns: self.column_count,
        };
        if let Some((cached_key, bands)) = self.item_bands_cache.borrow().as_ref() {
            if *cached_key == key {
                return Rc::clone(bands);
            }
        }
        let bands = Rc::new(self.compute_item_bands());
        *self.item_bands_cache.borrow_mut() = Some((key, Rc::clone(&bands)));
        bands
    }

    /// Rebuilds the row boxes; [`Self::item_bands`] is the cached entry point.
    fn compute_item_bands(&self) -> Vec<Rect> {
        if self.items.is_empty() {
            return Vec::new();
        }
        // Direct arithmetic, not a `FlexLayout` column: every row has a fixed height the control
        // itself owns (`item_height` / `separator_height`), so a layout engine was asked to
        // rediscover a constant on **every** draw — and during the open reveal that is once per
        // frame. Measured as menu-open latency on the GTK demo. The relations the layout used to
        // state are the same three lines below: the run starts at the popup's padding, each row
        // follows the previous one, and the run is as tall as `popup_height` reports.
        let padding = dimensions::MENU_POPUP_PADDING;
        let available = self.geometry().width.saturating_sub(padding * 2);
        let top = padding as i32 + self.tearoff_band_height() as i32;

        if self.column_count > 1 {
            // Row-major grid: entry `i` owns cell `(i / columns, i % columns)`. Every cell is
            // `item_height` tall, including a separator's, so the columns stay aligned; the
            // single-column branch keeps the historical separator height.
            let columns = self.column_count as u32;
            let gaps = MENU_COLUMN_GAP.saturating_mul(columns - 1);
            let column_width = available.saturating_sub(gaps) / columns;
            let column_width = column_width.max(1);
            let first_row = (self.scroll_offset / columns as usize) as i32;
            let mut placed = Vec::with_capacity(self.items.len());
            for index in 0..self.items.len() {
                let row = (index / columns as usize) as i32 - first_row;
                let column = (index % columns as usize) as u32;
                let x = padding as i32 + (column * (column_width + MENU_COLUMN_GAP)) as i32;
                let y = top + row * Self::item_height() as i32;
                placed.push(Rect::new(x, y, column_width, Self::item_height() as u32));
            }
            return placed;
        }

        let mut placed = Vec::with_capacity(self.items.len());
        // Rows above the window are laid out too (with negative `y`) rather than omitted:
        // the painter, the hit test and the keyboard all address an entry by its **list**
        // index, so a band that disappeared would shift every index below it.
        let skipped: i32 = self
            .items
            .iter()
            .take(self.scroll_offset)
            .map(|item| {
                if item.is_separator() {
                    Self::separator_height() as i32
                } else {
                    Self::item_height() as i32
                }
            })
            .sum();
        let mut y = top - skipped;
        for item in self.items.iter() {
            let height = if item.is_separator() {
                Self::separator_height() as u32
            } else {
                Self::item_height() as u32
            };
            placed.push(Rect::new(padding as i32, y, available, height));
            y += height as i32;
        }
        placed
    }

    /// The mnemonic presentation of every row, derived once per list/font revision.
    ///
    /// Stripping the `&` marker and measuring the prefix used to happen for every visible row on
    /// every frame; both are pure functions of the label and the resolved font, so the result is
    /// cached and invalidated by the same revision that invalidates the row boxes (G-P06).
    fn mnemonic_rows(&self, font: &crate::core::Font) -> Rc<Vec<MnemonicRow>> {
        let font_bits = font.size().to_bits();
        if let Some((version, cached_bits, rows)) = self.mnemonic_cache.borrow().as_ref() {
            if *version == self.items_version && *cached_bits == font_bits {
                return Rc::clone(rows);
            }
        }
        let rows: Vec<MnemonicRow> = self
            .items
            .iter()
            .map(|item| {
                let text = strip_menu_mnemonic(item.text());
                let underline = item.mnemonic().and_then(|mnemonic| {
                    text.char_indices().find(|(_, ch)| ch.eq_ignore_ascii_case(&mnemonic)).map(
                        |(byte, ch)| {
                            let prefix = estimate_text_width(&text[..byte], font, 1.0);
                            let through =
                                estimate_text_width(&text[..byte + ch.len_utf8()], font, 1.0);
                            (mnemonic, prefix, through.saturating_sub(prefix))
                        },
                    )
                });
                MnemonicRow { text, underline }
            })
            .collect();
        let rows = Rc::new(rows);
        *self.mnemonic_cache.borrow_mut() = Some((self.items_version, font_bits, Rc::clone(&rows)));
        rows
    }

    /// The deepest nested panel's rectangle, in the menu's own coordinate space.
    ///
    /// It hangs from the parent row's top edge and the parent level's trailing edge. The
    /// panel width is fixed at [`Self::submenu_panel_width`] so the panel cannot shrink
    /// around a short child label.
    pub fn submenu_panel_rect(&self) -> Option<Rect> {
        self.submenu_panel_rect_at(self.submenu_depth())
    }

    /// The width the nested panels paint at.
    pub fn submenu_panel_width() -> u32 {
        180
    }

    /// The height a nested panel needs for `children`.
    fn submenu_panel_height(children: &[MenuEntry]) -> u32 {
        children.iter().map(Self::entry_height).sum::<u32>() + 4
    }

    /// The height one panel row occupies.
    fn entry_height(entry: &MenuEntry) -> u32 {
        if entry.is_separator() || entry.is_section() {
            Self::separator_height() as u32
        } else {
            Self::item_height() as u32
        }
    }

    /// The panel rectangle of `level`, which is `1`-based, in the menu's coordinate space.
    pub(super) fn submenu_panel_rect_at(&self, level: usize) -> Option<Rect> {
        if level == 0 {
            return None;
        }
        let step = self.submenu_path.get(level - 1)?;
        let anchor = self.row_rect_at(level - 1, step.row)?;
        let width = Self::submenu_panel_width();
        // Every panel hangs 6 px over its parent's trailing edge, the same overlap the first
        // level has always used. Keeping the relation for each level is what makes a
        // four-deep cascade one shape rather than four hardcoded offsets.
        let parent =
            if level == 1 { self.geometry() } else { self.submenu_panel_rect_at(level - 1)? };
        let mut x = parent.x + parent.width as i32 - 6;
        let height = Self::submenu_panel_height(self.level_entries(level)?);
        let mut y = anchor.y;
        // A panel that would leave the surface is flipped to its parent's leading edge
        // when that side has room, and clamped inside the surface otherwise. Without
        // this, a branch near the right or bottom edge painted the rest of the cascade
        // off-screen (M-006).
        if let Some(viewport) = self.viewport {
            let right = viewport.x + viewport.width as i32;
            let bottom = viewport.y + viewport.height as i32;
            if x + width as i32 > right {
                let flipped = parent.x - width as i32 + 6;
                x = if flipped >= viewport.x {
                    flipped
                } else {
                    (right - width as i32).max(viewport.x)
                };
            }
            x = x.max(viewport.x);
            if y + height as i32 > bottom {
                y = (bottom - height as i32).max(viewport.y);
            }
            y = y.max(viewport.y);
        }
        Some(Rect::new(x, y, width, height))
    }

    /// Row `row` of `level` in the menu's coordinate space.
    pub(super) fn row_rect_at(&self, level: usize, row: usize) -> Option<Rect> {
        if level == 0 {
            let band = *self.item_bands().get(row)?;
            return Some(Rect::new(
                self.geometry().x + band.x,
                self.body_top() + band.y,
                band.width,
                band.height,
            ));
        }
        let panel = self.submenu_panel_rect_at(level)?;
        let entries = self.level_entries(level)?;
        let mut y = panel.y + 2;
        for (index, entry) in entries.iter().enumerate() {
            let height = Self::entry_height(entry);
            if index == row {
                return Some(Rect::new(panel.x + 2, y, panel.width.saturating_sub(4), height));
            }
            y += height as i32;
        }
        None
    }

    /// The child row at `pos` across every open panel, deepest first.
    ///
    /// Returns `(level, index)`, where `level` is `1` for the panel attached to the menu's own
    /// list. Panels are laid out side by side, so only one can contain the point; asking
    /// deepest-first keeps the answer honest if that ever changes.
    pub(super) fn hit_test_submenu(&self, pos: Point) -> Option<(usize, usize)> {
        for level in (1..=self.submenu_path.len()).rev() {
            let Some(panel) = self.submenu_panel_rect_at(level) else {
                continue;
            };
            if pos.x < panel.x
                || pos.x >= panel.x + panel.width as i32
                || pos.y < panel.y
                || pos.y >= panel.y + panel.height as i32
            {
                continue;
            }
            let Some(entries) = self.level_entries(level) else {
                continue;
            };
            let mut y = panel.y + 2;
            for (index, entry) in entries.iter().enumerate() {
                let height = Self::entry_height(entry) as i32;
                if pos.y >= y && pos.y < y + height {
                    return Some((level, index));
                }
                y += height;
            }
        }
        None
    }

    /// The child row at `pos`, when an open panel contains it.
    pub fn submenu_index_at(&self, pos: Point) -> Option<usize> {
        self.hit_test_submenu(pos).map(|(_, index)| index)
    }

    /// Paints every open nested panel.
    ///
    /// One panel per open level, hung from its parent row's right edge, deepest last. The rows
    /// use the same indicator/label/trailing derivations as the parent, so a child row aligns
    /// with a parent row even though the panel is narrower.
    #[allow(clippy::too_many_arguments)]
    fn draw_submenu_panel(
        &self,
        context: &mut RenderContext,
        font: &crate::core::Font,
        ink: Color,
        secondary: Color,
        primary: Color,
        border: Color,
        face: Color,
    ) {
        // Every open level paints its own panel, left to right. A panel is laid out from the
        // rows it actually holds, so a section or a separator keeps its own height and the
        // next level anchors to the row the user sees.
        for level in 1..=self.submenu_path.len() {
            let Some(panel) = self.submenu_panel_rect_at(level) else {
                continue;
            };
            let Some(children) = self.level_entries(level) else {
                continue;
            };
            context.fill_rect(panel, face.blend(&ink, 0.16));
            context.draw_rect(panel, border);
            // Content stays inside its own panel: a long child label or a wide icon must clip at
            // the panel edge instead of painting over the level beside it (M-02).
            context.push_clip(panel.x, panel.y, panel.width, panel.height);
            let mut y = panel.y + 2;
            for (index, child) in children.iter().enumerate() {
                let height = if child.is_separator() || child.is_section() {
                    Self::separator_height() as u32
                } else {
                    Self::item_height() as u32
                };
                let row = Rect::new(panel.x + 2, y, panel.width.saturating_sub(4), height);
                if child.is_separator() {
                    context.draw_line(
                        Point::new(row.x + 2, y + height as i32 / 2),
                        Point::new(row.x + row.width as i32 - 2, y + height as i32 / 2),
                        border,
                    );
                    y += height as i32;
                    continue;
                }
                if child.is_section() {
                    let band = Rect::new(row.x + 8, row.y, row.width.saturating_sub(8), row.height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        child.text(),
                        font,
                        secondary,
                        HorizontalAlignment::Left,
                    );
                    y += height as i32;
                    continue;
                }
                let hovered = self.level_hovered(level) == Some(index);
                if hovered {
                    context.fill_rect(row, primary);
                }
                let fg = if !child.is_enabled() {
                    self.base.disabled_ink_on(ink, face)
                } else if hovered {
                    primary.contrast_color()
                } else {
                    ink
                };
                let indicator = self.indicator_box(row);
                let label = self.label_box(row, indicator);
                if let Some(source) = child.icon() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1).min(indicator.width.max(1));
                    let icon_rect = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    if let Err(error) =
                        crate::widget::paint_icon(context, source.into(), icon_rect, fg)
                    {
                        log::warn!("submenu entry icon could not be painted: {error:?}");
                    }
                } else if child.is_checkable() && child.is_checked() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1);
                    let tick = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    crate::widget::draw_icon_at(context, tick, fg, IconName::Check);
                }
                context.draw_text_fitted_cow(
                    context.text_line(label, font),
                    child.text(),
                    font,
                    fg,
                    HorizontalAlignment::Left,
                );
                if child.has_submenu() && level < MAX_SUBMENU_DEPTH {
                    let arrow_box = self.submenu_arrow_box(row, label);
                    let side = arrow_box.width.min(arrow_box.height).min(16);
                    crate::widget::draw_icon_centered(
                        context,
                        arrow_box,
                        side,
                        fg,
                        IconName::ChevronRight,
                    );
                }
                y += height as i32;
            }
            context.pop_clip();
        }
    }
    pub(super) fn separator_height() -> f32 {
        6.0
    }

    /// The box the check/radio indicator occupies at a row's leading edge.
    ///
    /// # Why the indicator is a box and not an `8.0` literal
    ///
    /// The check column, the label column and the two trailing columns used to be four
    /// independent literals in `draw` (`x + 8`, `x + 28`, `right - 8`, `right - 4`), which is
    /// the exact shape BLUE22 §B.8 lists for this control: with literals, an indicator or an
    /// arrow that changes size does not *push* its neighbour, it overlaps it. The standard relation is stated
    /// directly —
    /// `leftPadding: padding + (checkable ? indicator.width + spacing : 0)` — so the label's
    /// box is whatever the indicator leaves, and the shortcut's box is whatever the submenu
    /// arrow leaves.
    pub(super) fn indicator_box(&self, row: Rect) -> Rect {
        let size = dimensions::CHECKBOX_BOX.min(row.width).min(row.height);
        Rect::new(
            row.x + dimensions::MENU_ROW_PADDING_H as i32,
            row.y + (row.height as i32 - size as i32) / 2,
            size,
            size,
        )
    }

    /// The box a row's label may occupy: the row's interior after the indicator column.
    ///
    /// Derived from [`Self::indicator_box`], so a wider indicator narrows the label rather
    /// than letting the two overlap. The column is reserved for **every** row, checkable or
    /// not, because a menu whose labels shift sideways depending on whether a *sibling* has a
    /// tick is the classic menu misalignment.
    pub(super) fn label_box(&self, row: Rect, indicator: Rect) -> Rect {
        let left = indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32;
        let right = self.trailing_column_x(row);
        Rect::new(left, row.y, right.saturating_sub(left).max(0) as u32, row.height)
    }

    /// The x where the trailing columns (shortcut, submenu arrow) begin.
    pub(super) fn trailing_column_x(&self, row: Rect) -> i32 {
        row.x + row.width as i32 - self.trailing_width() as i32
    }

    /// Width of the column the shortcut / submenu arrow occupies.
    ///
    /// The constant (`MENU_ROW_TRAILING_WIDTH`) is the *minimum*, sized for a submenu arrow.
    /// A shortcut is text and can be wider than that — `Ctrl+Shift+Z` at body size is three
    /// times the arrow's column — and the old fixed reserve handed `draw_text_fitted` a box
    /// narrower than the string, so every shortcut in the demo painted as "…". Measuring the
    /// widest shortcut in **this** menu keeps the column honest for the entries it actually
    /// holds, and keeps the label box from stealing the text's room.
    pub(super) fn trailing_width(&self) -> u32 {
        let font = effective_font(self.style());
        let font_bits = font.size().to_bits();
        if let Some((version, cached_bits, width)) = self.trailing_width_cache.get() {
            if version == self.items_version && cached_bits == font_bits {
                return width;
            }
        }
        let widest = self
            .items
            .iter()
            .filter(|item| self.row_shows_shortcut(item))
            .map(|item| {
                estimate_text_width(item.shortcut(), font, 1.0) + 2 * crate::render::TEXT_FIT_MARGIN
            })
            .max()
            .unwrap_or(0);
        let width = widest.max(dimensions::MENU_ROW_TRAILING_WIDTH);
        self.trailing_width_cache.set(Some((self.items_version, font_bits, width)));
        width
    }

    /// The box the submenu arrow may occupy: what is left of the row after the label.
    ///
    /// Derived from the **label's own right edge** rather than from the row's trailing column, so a
    /// longer label shortens the arrow's room instead of the two being placed from opposite ends of
    /// the row and colliding in the middle. Extracted from the draw loop so the layout this test
    /// asserts and the layout that draws are one derivation (`principle #23`), and so a row wide
    /// enough for a shortcut *and* an arrow can be checked as a set of disjoint columns.
    pub(super) fn submenu_arrow_box(&self, row: Rect, label: Rect) -> Rect {
        let left = label.x + label.width as i32;
        Rect::new(left, row.y, (row.x + row.width as i32 - left).max(0) as u32, row.height)
    }

    /// Whether `item`'s row shows its shortcut in the trailing column.
    ///
    /// # Why this predicate exists
    ///
    /// The trailing column is **shared** by the shortcut and the submenu arrow: one 28 px reserve
    /// rather than two, because a row was documented to show at most one of them at a time
    /// (`dimensions::MENU_ROW_TRAILING_WIDTH`). Nothing enforced that, and the draw path placed the
    /// shortcut right-aligned and the arrow centred in the *same* box — so an entry given both drew
    /// them on top of each other. A row is not an unusual thing to configure that way: a submenu entry
    /// with a shortcut is legal to construct.
    ///
    /// The arrow wins, and the reason is asymmetric: the arrow is the affordance that says "this
    /// reveals a subtree", and hiding it makes the subtree unreachable by pointer, whereas the
    /// constant's own documentation records that a shortcut on such a row is not useful. Stating the
    /// precedence here means the layout, the draw and the test read one rule instead of three.
    pub(super) fn row_shows_shortcut(&self, item: &MenuEntry) -> bool {
        !item.shortcut().is_empty() && !item.has_submenu()
    }
    fn _item_rect(&self, index: usize, base_y: f32) -> Rect {
        let rect = self.geometry();
        let mut y = base_y;
        for (i, item) in self.items.iter().enumerate() {
            let h =
                if item.is_separator() { Self::separator_height() } else { Self::item_height() };
            if i == index {
                return Rect { x: rect.x, y: y as i32, width: rect.width, height: h as u32 };
            }
            y += h;
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
    pub(super) fn popup_height(&self) -> f32 {
        // The height is a pure function of the visible window and the row kinds, so it is
        // cached against the list revision and the geometry that selects the window. Opening
        // a menu while its reveal animates read this on every frame (G-P01).
        let key = PopupMetricsKey {
            version: self.items_version,
            width: self.geometry().width,
            columns: self.column_count,
            tearoff: self.tearoff,
            capacity: Self::visible_capacity(),
            font_bits: effective_font(self.style()).size().to_bits(),
        };
        if let Some((cached_key, height)) = self.popup_metrics_cache.get() {
            if cached_key == key {
                return height;
            }
        }
        let height = self.compute_popup_height();
        self.popup_metrics_cache.set(Some((key, height)));
        height
    }

    /// Rebuilds the popup height; [`Self::popup_height`] is the cached entry point.
    fn compute_popup_height(&self) -> f32 {
        let range = self.visible_range();
        let tearoff = self.tearoff_band_height() as f32;
        if self.column_count > 1 {
            let visible = range.len();
            let rows = visible.div_ceil(self.column_count as usize);
            return rows as f32 * Self::item_height() + 4.0 + tearoff;
        }
        self.items[range]
            .iter()
            .map(
                |item| {
                    if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    }
                },
            )
            .sum::<f32>()
            + 4.0
            + tearoff
    }

    /// Paints the popup's elevation shadow.
    ///
    /// `blur` in [`crate::style::MenuEffects`] is the shadow's blur radius: the renderer has
    /// no way to sample the backdrop, so a backdrop blur is not available (the same boundary
    /// `Material::Translucent` documents). Without hardware acceleration the shadow is a flat
    /// offset rect; the blurred path is the expensive one that flag gates.
    /// The entry drawn at `pos`, if any.
    ///
    /// Uses the same bands the painter uses, which is what keeps a click on the row the user
    /// can see from landing on the row above it after a scroll.
    pub(super) fn item_index_at(&self, pos: Point) -> Option<usize> {
        if !self.contains_point(pos) {
            return None;
        }
        let bands = self.item_bands();
        let body_top = self.body_top();
        let left = self.geometry().x;
        for index in self.visible_range() {
            let Some(row) = bands.get(index) else { continue };
            let top = body_top + row.y;
            // A single-column menu historically addressed a row by its y band alone, which is
            // what makes the popup's horizontal padding part of the row. A grid has real
            // columns, so it must test x as well or every column would answer for the first.
            let hit = if self.column_count > 1 {
                pos.x >= left + row.x
                    && pos.x < left + row.x + row.width as i32
                    && pos.y >= top
                    && pos.y < top + row.height as i32
            } else {
                pos.y >= top && pos.y < top + row.height as i32
            };
            if hit {
                return Some(index);
            }
        }
        None
    }
}
impl Draw for Menu {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The effect policy is read once per frame. With `menu_config` disabled this is
        // `MenuEffects::legacy()`, so every paint below takes the pre-wiring path.
        let effects = effects::current();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the popup unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, and the palette comes from the shared snapshot, so no guard is
        // held across the draw — the global manager's mutex is not re-entrant.
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — used for the heading, every
        // entry label and every shortcut, so the menu honours the theme body font and the user's
        // text scale (D09-STYLE-01).
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("menu");
        // Read once from the palette snapshot and copied out as values; `palette_shared()` holds
        // no manager guard, so the manager is free for the rest of the draw.
        let (window_fill, foreground, secondary, primary) = {
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.background, colors.foreground, colors.secondary, colors.primary)
                }
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `menu` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as `Surface`
        // and the active theme writes the window fill into `style.background_color`. A popup
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, while a
        // colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The heading is what makes a closed menu visible.
        //
        // A menu is born hidden, and `draw` used to `return` immediately in that state, so a
        // freshly constructed menu painted nothing and the census reported `ink = 0`. The title
        // strip is drawn in both states, which reads as the popup's heading when open and as the
        // control's own idle label when closed — and makes `title()`'s documented "drawn as its
        // heading" contract true, which it previously was not: nothing on this type drew it.
        let heading_h = self.drawn_heading_height();
        if heading_h > 0.0 {
            let heading_fill = effects::popup_face(face.blend(&ink, 0.10), &effects);
            context
                .fill_rect(Rect::new(rect.x, rect.y, rect.width, heading_h as u32), heading_fill);
            // The origin is the glyph's **top** edge, so the heading's vertical centre is half the
            // difference between the strip and the line box. Passing the strip's midline put the
            // glyph's top *at* the centre, so a 14 px label in a 20 px heading ended at y = 24 —
            // four pixels below the strip it belongs to.
            // The heading line height is one shaped measurement and is a function of the font
            // alone, so it is cached unconditionally. Gating the cache on `effects.caching`
            // made the default policy pay the measurement on every frame of the open reveal.
            let heading_text_h = {
                let key = font.size().to_bits();
                match self.heading_line_cache.get() {
                    Some((cached_key, height)) if cached_key == key => height,
                    _ => {
                        let height = context.measure_text("M", font).height;
                        self.heading_line_cache.set(Some((key, height)));
                        height
                    }
                }
            };
            context.draw_text(
                Point::new(rect.x + 8, rect.y + (heading_h as i32 - heading_text_h as i32) / 2),
                &self.title,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            context.draw_line(
                Point::new(rect.x, rect.y + heading_h as i32),
                Point::new(rect.x + rect.width as i32, rect.y + heading_h as i32),
                border,
            );
        }

        // # Why the body is painted while it is *either* open or still closing
        //
        // `is_visible` is the authority and flips on the frame the menu opens or closes, but it is
        // a `bool` and cannot describe a panel halfway. The body is therefore drawn whenever the
        // reveal has anything left to show — which is the open state **and** the frames of a
        // closing one. Testing the flag alone would cut the closing animation off at its first
        // frame, i.e. exactly the hard cut this control was fixing.
        if !self.is_visible() && self.reveal.is_closed() {
            return;
        }

        let popup_y = rect.y + heading_h as i32;
        let popup_h = self.popup_height();
        // The body the entries are painted into, revealed from the heading downward so the menu
        // appears to grow out of its title strip. The clip is what makes the reveal honest: a
        // body that only shrank its background would still print entries outside its own edge, so
        // a user would see rows the geometry has not admitted to yet.
        let body = Rect::new(rect.x, popup_y, rect.width, popup_h as u32);
        let revealed = self.reveal.revealed(
            body,
            RevealDirection::Down,
            PopupReveal::travel_cap_for(crate::core::Size::new(body.width, body.height)),
        );
        // The shadow is painted **before** the clip: it extends outside the body by design,
        // so clipping it to the panel would hide the only part a user can see.
        if effects.shadows {
            effects::paint_popup_shadow(context, revealed, &effects);
        }
        // The clip is unconditional now: with scrolling a row can sit outside the body even
        // when the reveal has settled, and the clip is what keeps it invisible.
        context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
        let body_fill = effects::popup_face(face.blend(&ink, 0.22), &effects);
        context.fill_rect(Rect::new(rect.x, popup_y, rect.width, popup_h as u32), body_fill);
        context.draw_rect(Rect::new(rect.x, popup_y, rect.width, popup_h as u32), border);
        // The tear-off strip is a row of dots: the drag affordance a detached menu shows at
        // its top edge. It is painted only when the caller asked for it.
        let tearoff_band = self.tearoff_band_height();
        if tearoff_band > 0 {
            let y = popup_y + tearoff_band as i32 / 2;
            let mut x = rect.x + 6;
            let right = rect.x + rect.width as i32 - 6;
            while x + 2 <= right {
                context.fill_rect(Rect::new(x, y, 2, 1), border);
                x += 4;
            }
        }
        // The run of rows, placed by the layout. `bands` is in popup coordinates, so it is offset by
        // the popup's own top edge once here rather than accumulated per row.
        let bands = self.item_bands();
        // Mnemonic labels are derived once per list/font revision and read per row (G-P06).
        let mnemonic_rows = if self.show_mnemonics { Some(self.mnemonic_rows(font)) } else { None };
        for i in self.visible_range() {
            let Some(item) = self.items.get(i) else { continue };
            let Some(row) = bands.get(i).copied() else { continue };
            let row = Rect::new(rect.x + row.x, popup_y + row.y, row.width, row.height);
            if item.is_separator() || item.is_section() {
                if item.is_separator() {
                    let sep_y = row.y + row.height as i32 / 2;
                    context.draw_line(
                        Point::new(rect.x + 4, sep_y),
                        Point::new(rect.x + rect.width as i32 - 4, sep_y),
                        border,
                    );
                } else {
                    // A section caption is a quiet label, not a choice, so it is drawn in the
                    // secondary ink and never takes the highlight fill.
                    let band =
                        Rect::new(row.x + 10, row.y, row.width.saturating_sub(20), row.height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        item.text(),
                        font,
                        secondary,
                        HorizontalAlignment::Left,
                    );
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            if is_hovered {
                context.fill_rect(
                    Rect::new(row.x + 2, row.y, row.width.saturating_sub(4), row.height),
                    primary,
                );
            }
            // A disabled entry is dimmed, and a highlighted one takes the contrast colour of the
            // primary it sits on, so neither is a fixed grey or a fixed white that only read on a
            // light popup.
            let fg = if !item.is_enabled() {
                self.base.disabled_ink_on(ink, face)
            } else if is_hovered {
                primary.contrast_color()
            } else {
                ink
            };
            // The row every text run and indicator below is placed in, and the two boxes they
            // are centred in. Deriving all three here means the check column, the label column
            // and the shortcut column are readings of one row rather than four literals, and
            // the glyph origins come from `text_line` rather than from `y + height / 2`.
            //
            // The latter was the visible defect: `draw_text`'s origin is the glyph box's
            // **top** edge, so `y + item_height() / 2` put that edge on the row's middle line
            // and drew every entry label, tick, shortcut and arrow half a line low — the same
            // error the heading directly above already documented and avoided.
            //
            // The row's own box comes from `item_bands`, so this loop no longer carries a `y`.
            let indicator = self.indicator_box(row);
            let label = self.label_box(row, indicator);
            if let Some(source) = item.icon() {
                // An icon occupies the indicator column, so a checkable row that also carries
                // an icon shows the icon rather than two marks competing for one box.
                let line = context.text_line(indicator, font);
                let side = line.height.max(1).min(indicator.width.max(1));
                let icon_rect = Rect::new(
                    indicator.x + (indicator.width as i32 - side as i32) / 2,
                    line.y,
                    side,
                    side,
                );
                if let Err(error) = crate::widget::paint_icon(context, source.into(), icon_rect, fg)
                {
                    log::warn!("menu entry icon could not be painted: {error:?}");
                }
            } else if item.is_checkable() {
                // The tick is the `Check` **icon outline**, not the `✓` text glyph (U+2713 is
                // covered by no bundled face, so it drew as an 8x8 bitmap block). An unchecked row
                // draws nothing, which is what the old blank-space string achieved.
                if item.is_checked() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1);
                    let tick = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    crate::widget::draw_icon_at(context, tick, fg, IconName::Check);
                }
            }
            // Mnemonic markers are metadata until the caller asks for them: with the flag
            // off the raw label is drawn, byte for byte as before.
            let label_text = match mnemonic_rows.as_ref().and_then(|rows| rows.get(i)) {
                Some(row) => row.text.as_str(),
                None => item.text(),
            };
            let line = context.text_line(label, font);
            context.draw_text_fitted_cow(line, label_text, font, fg, HorizontalAlignment::Left);
            if let Some((_, prefix, width)) =
                mnemonic_rows.as_ref().and_then(|rows| rows.get(i)).and_then(|row| row.underline)
            {
                // The underline is placed from the **cached** measured prefix, so it tracks the
                // glyph the marker names without re-measuring the label on every frame.
                let underline = Rect::new(
                    line.x + prefix as i32,
                    line.y + line.height as i32 - 1,
                    width.max(1),
                    1,
                );
                context.fill_rect(underline, fg);
            }
            if self.row_shows_shortcut(item) {
                // One line box for the shortcut and the arrow: they occupy one trailing column,
                // so they cannot disagree about where that column's midline is — and only one of
                // them is ever drawn in it (`row_shows_shortcut`).
                let trailing = Rect::new(
                    self.trailing_column_x(row),
                    row.y,
                    self.trailing_width(),
                    row.height,
                );
                context.draw_text_fitted_cow(
                    context.text_line(trailing, font),
                    item.shortcut(),
                    font,
                    fg,
                    HorizontalAlignment::Right,
                );
            }
            if item.has_submenu() {
                // Derived from the label box's own right edge (see `submenu_arrow_box`), so a longer
                // label shortens the room the arrow has instead of the two being placed from opposite
                // ends of the row and colliding in the middle. The mark is the `ChevronRight` **icon
                // outline**, not the `▶` text glyph (U+25B6 is covered by no bundled face, so it drew
                // as an 8x8 bitmap block).
                let arrow_box = self.submenu_arrow_box(row, label);
                let side = arrow_box.width.min(arrow_box.height).min(16);
                crate::widget::draw_icon_centered(
                    context,
                    arrow_box,
                    side,
                    fg,
                    IconName::ChevronRight,
                );
            }
        }
        // Popped on the same path it was pushed, so the reveal cannot leak a clip onto whatever
        // the host draws after this menu.
        context.pop_clip();
        // The nested panel is painted after the parent's clip is popped: it hangs outside the
        // parent rectangle by design, so clipping it to the parent would hide all of it.
        if self.is_visible() || !self.reveal.is_closed() {
            self.draw_submenu_panel(context, font, ink, secondary, primary, border, face);
        }

        // The hover hint is drawn last, over the row it explains and any open child panel.
        let geometry = self.geometry();
        self.tooltips.set_geometry(geometry);
        self.tooltips.draw(context);
    }
}
