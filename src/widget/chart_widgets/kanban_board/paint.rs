// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

impl Draw for KanbanBoard {
    fn draw(&mut self, context: &mut RenderContext) {
        // Keep the attached proxy on the board's drag session; a drag can end or move without an
        // event this control receives before painting.
        let _ = self.poll_drag_ghost_layer();
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("kanban_board");
        // `kanban_board` is not a control kind in the role table, so it classifies as
        // `Surface` — whose background is `theme.colors.background`, the very colour a
        // window paints. Filling the whole rect with it would make the board
        // indistinguishable from the window behind it (the census reports that as
        // "painted nothing"), so the board's own fill is a step toward the foreground
        // and the window keeps its colour.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // Every colour the board paints below is derived from this one resolved
        // triple, so a theme switch moves the whole board rather than the frame
        // only. The column, its header strip, its cards and the drag feedback are
        // all chrome states of the same surface.
        let chrome = BoardChrome { background, border, text_color };

        // One face per role for the whole board: these were resolved inside `draw_column`, so a
        // board of N columns re-derived (and re-allocated the family string of) four fonts per
        // column per frame.
        let fonts = ColumnFonts {
            header: role_font(self.base.style(), 13.0),
            badge: role_font(self.base.style(), 11.0),
            title: role_font(self.base.style(), 12.0),
            desc: role_font(self.base.style(), 10.0),
        };

        // Columns that the horizontal scroll has moved past either edge are skipped, and the rest
        // are clipped to the board: a column laid out at a negative x would otherwise paint over
        // whatever the layout put to the left of the board (nothing clips a widget at this layer).
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        for index in 0..self.columns.len() {
            let Some(column_rect) = self.column_rect(index) else {
                continue;
            };
            if column_rect.x + column_rect.width as i32 <= rect.x
                || column_rect.x >= rect.x + rect.width as i32
            {
                continue;
            }
            self.draw_column(context, index, &chrome, &fonts);
        }
        if self.lanes_active() {
            self.draw_lane_headers(context, &chrome, &fonts);
        }
        context.pop_clip();
        // The drag overlay is painted last so the card follows the pointer over
        // every column rather than being clipped to the one it started in. It is clamped to the
        // board inside `draw_drag_overlay`, so it stays inside the control without a clip.
        self.draw_drag_overlay(context, &chrome);
    }
}

/// The board's resolved chrome colours, threaded through the drawing helpers.
///
/// Gathered into one value so every column, card and drag indicator reads the
/// same appearance-resolved colours instead of re-resolving the theme per row,
/// and so no helper can drift back to a literal.
#[derive(Debug, Clone, Copy)]
pub(super) struct BoardChrome {
    /// The board's own fill, and the base every other colour is derived from.
    background: Color,
    /// The resolved border colour, used for card outlines.
    border: Color,
    /// The resolved foreground, used for text and as the tint direction.
    text_color: Color,
}

/// The four faces a column draws with, resolved once per paint.
///
/// `role_font` re-derives the font from the style (cloning its family string), so resolving
/// these inside `draw_column` cost four allocations per column per frame.
pub(super) struct ColumnFonts {
    header: crate::core::Font,
    badge: crate::core::Font,
    title: crate::core::Font,
    desc: crate::core::Font,
}

impl BoardChrome {
    /// A column's fill: the `surface_container` role when the theme offers one, so a theme can
    /// tune the lane colour independently of the board it sits on.
    ///
    /// # Why the role comes first (BLUE23 §93, M5)
    ///
    /// The lane was a local blend of the board's own colour, which is a value no theme can address:
    /// changing the board moved the lanes with it, so "the board is dark but its lanes should be
    /// lighter" was not expressible. `surface_container` is the role the crate already declares for
    /// exactly this — one step up from the page — and it is what `panel`, `group_box` and
    /// `emoji_picker` read for the same purpose. The blend survives as the fallback for a theme
    /// that predates the role.
    ///
    /// It still steps *away* from the board rather than adopting the role verbatim when the role
    /// has resolved to the board's own colour, which is the same guard `drop_zone` and `otp_input`
    /// use: a lane indistinguishable from the board is not a lane.
    fn column(&self) -> Color {
        let role = crate::style::layer_color(crate::style::LayerColor::SurfaceContainer);
        match role {
            Some(role) if role != self.background => role,
            _ => self.background.blend(&self.text_color, 0.18),
        }
    }

    /// The column a dragged card is hovering over, tinted toward the text colour
    /// so the drop target is visible in either appearance.
    fn drop_target(&self) -> Color {
        self.background.blend(&self.text_color, 0.3)
    }

    /// The column header strip, a second step away from the column body.
    fn header(&self) -> Color {
        self.background.blend(&self.text_color, 0.24)
    }

    /// Header text, and the ordinary card title.
    fn text(&self) -> Color {
        self.text_color
    }

    /// The card count and other secondary labels.
    fn muted_text(&self) -> Color {
        self.text_color.blend(&self.background, 0.3)
    }

    /// The card surface, one step toward the text colour from the board so a card
    /// reads as raised above its column.
    fn card(&self) -> Color {
        self.background.blend(&self.text_color, 0.1)
    }

    /// One lane band, alternating by parity so two adjacent lanes read as separate bands even
    /// when no card happens to sit in one of them.
    fn lane_band(&self, index: usize) -> Color {
        let step = if index.is_multiple_of(2) { 0.18 } else { 0.13 };
        self.background.blend(&self.text_color, step)
    }

    /// A card that has been marked done: title text muted, like the count badge.
    fn done_text(&self) -> Color {
        self.text_color.blend(&self.background, 0.45)
    }

    /// The WIP limit is a *state* the user has to act on, so it reads the theme's
    /// error token rather than a literal red.
    fn at_limit(&self) -> Color {
        crate::style::semantic_color(crate::style::SemanticColor::Error)
            .map(|token| token.blend(&self.background, 0.2))
            .unwrap_or_else(|| self.text_color.blend(&self.background, 0.3))
    }

    /// The insertion indicator shown where a dragged card would land. This is the
    /// theme's primary slot, read from the board's resolved border colour so it
    /// moves with the appearance instead of staying a fixed blue.
    fn insertion(&self) -> Color {
        self.border.blend(&self.text_color, 0.3)
    }

    /// The colour a card's own drag ghost is filled with.
    fn ghost(&self) -> Color {
        self.background.blend(&self.text_color, 0.08)
    }
}

impl KanbanBoard {
    /// Draws one column: its background, header, cards and the drop preview.
    pub(super) fn draw_column(
        &mut self,
        context: &mut RenderContext,
        index: usize,
        chrome: &BoardChrome,
        fonts: &ColumnFonts,
    ) {
        let Some(column_rect) = self.column_rect(index) else {
            return;
        };
        // Borrowed rather than cloned: the tuple used to copy the title `String`, the whole
        // card vector (and every card's title/description) once per column per frame. Every
        // helper below (`column_rect`, `card_rect`, `draw_card`) takes `&self`, so the borrow
        // can live across the column.
        let Some(column) = self.columns.get(index) else {
            return;
        };
        let title = column.title.as_str();
        let collapsed = column.collapsed;
        let cards = &column.cards;
        let at_limit = column.is_at_wip_limit();
        let wip_limit = column.wip_limit;

        // The drop preview is drawn under the cards, so a highlighted column still
        // shows its contents.
        let is_drop_target = self.is_dragging_card() && self.hovered_column == Some(index);
        let background = if is_drop_target { chrome.drop_target() } else { chrome.column() };
        context.fill_rounded_rect(column_rect, 8, background);

        // Header height is `HEADER_HEIGHT`; the title and the count badge are centred in it
        // through the shared primitive rather than at a literal `+ 21`, which was a third of a
        // 32 px header and moved neither with the header nor with the font.
        let header_rect = Rect::new(column_rect.x, column_rect.y, column_rect.width, HEADER_HEIGHT);
        context.fill_rounded_rect(header_rect, 8, chrome.header());
        // A WIP limit is only worth showing when it is reached, which is the state a
        // user has to act on.
        let header_color = if at_limit { chrome.at_limit() } else { chrome.text() };
        let header_font = &fonts.header;
        let badge_font = &fonts.badge;
        let title_font = &fonts.title;
        let desc_font = &fonts.desc;
        // The collapse chevron is the header's leading cell, matching the standalone
        // `KanbanColumnHeader` control. Its box is also the board's click target.
        if let Some(icon_rect) = self.column_collapse_rect(index) {
            let chevron = if collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
            draw_icon_at(context, icon_rect, header_color, chevron);
        }
        let title_left = header_rect.x + 4 + HEADER_ICON + 6;
        let header_line = context.text_line(header_rect, header_font);
        // The badge is measured first so the title knows where to stop: an unbounded
        // title ran underneath the count on the right.
        let mut badge = String::new();
        if collapsed {
            let _ = write!(badge, "({})", cards.len());
        } else if let Some(limit) = wip_limit {
            let _ = write!(badge, "{}/{}", cards.len(), limit);
        } else {
            let _ = write!(badge, "{}", cards.len());
        }
        let badge_metrics = context.measure_text(&badge, badge_font);
        let badge_line = context.text_line(header_rect, badge_font);
        let badge_x =
            header_rect.x + header_rect.width as i32 - CARD_PADDING - badge_metrics.width as i32;
        let title_width = ((badge_x - 6) - title_left).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(title_left, header_line.y, title_width, header_line.height.max(1)),
            title,
            header_font,
            header_color,
            HorizontalAlignment::Left,
        );
        // Card count on the right of the header, or the collapsed marker. One buffer
        // reused per column: the three-branch `format!`/`to_string` chain allocated a
        // fresh `String` for every column on every frame.
        context.draw_text(
            Point::new(badge_x, badge_line.y),
            &badge,
            badge_font,
            chrome.muted_text(),
            HorizontalAlignment::Left,
        );

        if collapsed {
            return;
        }

        // In lane mode the band is the column's row grid: painting it here (rather than in the
        // board loop) keeps each band clipped to the column it belongs to, so a card cannot appear
        // to float in a neighbouring column.
        if self.lanes_active() {
            for lane in 0..self.lanes.len() {
                let Some(band) = self.lane_band_rect(lane) else {
                    continue;
                };
                let cell = Rect::new(column_rect.x, band.y, column_rect.width, band.height);
                context.fill_rect(cell, chrome.lane_band(lane));
                context.draw_line(
                    Point::new(cell.x, cell.y + cell.height as i32),
                    Point::new(cell.x + cell.width as i32, cell.y + cell.height as i32),
                    chrome.border,
                );
            }
        }

        // Clipping is a column-level state, not a card-level one: the column rectangle is what
        // bounds its cards, so one push/pop per column replaces one per card. Before this a screen
        // of a few hundred cards paid a clip transition for every card in the paint loop.
        context.push_clip(column_rect.x, column_rect.y, column_rect.width, column_rect.height);
        // Only the cards inside the column's scrolled window are painted: the column clip discards
        // everything outside it anyway, so walking the whole list made a long column pay per card
        // per frame for marks that could not reach the screen.
        let (first_card, last_card) = self.visible_card_range(index);
        // `enumerate` over the window gives the card's own index without indexing the slice: the
        // `for i in first..last { &cards[i] }` form is what `clippy::needless_range_loop` flags.
        for (card_index, card) in cards.iter().enumerate().take(last_card).skip(first_card) {
            let position = CardPosition { column: index, card: card_index };
            let Some(card_rect) = self.card_rect(position) else {
                continue;
            };
            self.draw_card(context, card, card_rect, position, chrome, title_font, desc_font);
        }
        context.pop_clip();
    }

    /// Draws the lane labels down the left edge and the divider that separates them from the
    /// columns. The bands themselves are painted per column by `draw_column`.
    pub(super) fn draw_lane_headers(
        &self,
        context: &mut RenderContext,
        chrome: &BoardChrome,
        fonts: &ColumnFonts,
    ) {
        let rect = self.geometry();
        let caption = Rect::new(rect.x, rect.y, LANE_HEADER_WIDTH, HEADER_HEIGHT);
        context.fill_rounded_rect(caption, 8, chrome.header());
        context.draw_text_line_cow(
            Rect::new(
                caption.x + CARD_PADDING,
                caption.y,
                caption.width.saturating_sub(2 * CARD_PADDING as u32),
                caption.height,
            ),
            "Lanes",
            &fonts.header,
            chrome.muted_text(),
            HorizontalAlignment::Left,
        );
        for index in 0..self.lanes.len() {
            let Some(header) = self.lane_header_rect(index) else {
                continue;
            };
            context.fill_rounded_rect(header, 6, chrome.lane_band(index));
            // The chevron is the lane's collapse control, in the same leading cell the column
            // headers use, so one interaction model covers both levels (SP-001).
            if let Some(icon_rect) = self.lane_collapse_rect(&self.lanes[index].id) {
                let icon = if self.is_lane_collapsed(&self.lanes[index].id) {
                    crate::widget::IconName::ChevronRight
                } else {
                    crate::widget::IconName::ChevronDown
                };
                crate::widget::draw_icon_at(context, icon_rect, chrome.text(), icon);
            }
            context.draw_text_line_cow(
                Rect::new(
                    header.x + CARD_PADDING + HEADER_ICON,
                    header.y,
                    header.width.saturating_sub((CARD_PADDING + HEADER_ICON) as u32 * 2),
                    LANE_HEADER_HEIGHT as u32,
                ),
                &self.lanes[index].title,
                &fonts.header,
                chrome.text(),
                HorizontalAlignment::Left,
            );
        }
        let divider_x = rect.x + LANE_HEADER_WIDTH as i32;
        context.draw_line(
            Point::new(divider_x, rect.y + HEADER_HEIGHT as i32),
            Point::new(divider_x, rect.y + rect.height as i32),
            chrome.border,
        );
    }

    /// Draws one card, marking it when it is the one being dragged.
    pub(super) fn draw_card(
        &self,
        context: &mut RenderContext,
        card: &KanbanCard,
        card_rect: Rect,
        position: CardPosition,
        chrome: &BoardChrome,
        title_font: &Font,
        desc_font: &Font,
    ) {
        let is_dragged = self.drag_origin == Some(position) && self.is_dragging_card();
        // A hovered card lifts off the column with a small shadow, so the pointer's target
        // reads as a raised object before it is dragged. The shadow is drawn first and the
        // card keeps its rectangle, which is what keeps the layout (and the hit test) still.
        if self.hovered_card == Some(position) && !self.is_dragging_card() {
            context.draw_box_shadow(card_rect, Color::BLACK.with_alpha(40), 0, 3, 6, 0);
        }
        // The dragged card stays drawn in place but faded, so the user can see both
        // where it came from and where it is going.
        let alpha = if is_dragged { 90 } else { 255 };
        context.fill_rounded_rect(card_rect, 6, chrome.card().with_alpha(alpha));
        let focused = self.focused_card == Some(position);
        context.draw_rounded_rect_stroke(
            card_rect,
            6,
            if focused { chrome.text().with_alpha(alpha) } else { chrome.border.with_alpha(alpha) },
            if focused { 2 } else { 1 },
        );
        // A selected card is a member of the batch a drop will move, so it reads as its own state
        // rather than as the keyboard focus ring.
        if self.selected_set.contains(card.id.as_str()) {
            context.draw_rounded_rect_stroke(card_rect, 6, chrome.insertion(), 2);
        }

        let title_color = if card.done {
            chrome.done_text().with_alpha(alpha)
        } else {
            chrome.text().with_alpha(alpha)
        };
        let title = card.title.as_str();
        // The title and the description are placed from the card's own box: the literals
        // `+ 20` and `+ 38` described a 48 px card, so any other `CARD_HEIGHT` drew the
        // description past the card's bottom edge. A card is `CARD_HEIGHT` tall by contract.
        let title_line = context.text_line(card_rect, title_font);
        // The done marker used to be a `✓` (U+2713) prefix on the title string. No bundled
        // face covers that code point, so the text path fell back to an 8x8 bitmap and drew a
        // blocky check mark. An `IconName::Check` is real geometry both backends agree on, and
        // drawing it in its own square keeps it independent of the label's glyph coverage.
        let mut title_x = card_rect.x + CARD_PADDING;
        if card.done {
            let icon_side: u32 = 10;
            let icon_rect = Rect::new(
                title_x,
                title_line.y + (title_line.height as i32 - icon_side as i32) / 2,
                icon_side,
                icon_side,
            );
            draw_icon_at(context, icon_rect, title_color, IconName::Check);
            title_x += icon_side as i32 + 4;
        }
        if !title.is_empty() {
            context.draw_text(
                Point::new(title_x, title_line.y),
                title,
                title_font,
                title_color,
                HorizontalAlignment::Left,
            );
        }
        if !card.description.is_empty() {
            // The description occupies the card's lower half, so the two lines never overlap
            // whatever the card's height is.
            let desc_band = Rect::new(
                card_rect.x,
                card_rect.y + (card_rect.height / 2) as i32,
                card_rect.width,
                card_rect.height / 2,
            );
            let desc_line = context.text_line(desc_band, desc_font);
            context.draw_text(
                Point::new(card_rect.x + CARD_PADDING, desc_line.y),
                &card.description,
                desc_font,
                chrome.muted_text().with_alpha(alpha),
                HorizontalAlignment::Left,
            );
        }
    }

    /// Draws the card following the pointer.
    ///
    /// Uses [`DropTarget::preview_rect`] on this board to find the landing column,
    /// so the preview and the commit read one answer rather than two rules.
    pub(super) fn draw_drag_overlay(&mut self, context: &mut RenderContext, chrome: &BoardChrome) {
        let Some(session) = self.drag.as_ref() else {
            return;
        };
        if !session.is_active() {
            return;
        }
        let current = session.current();
        let preview = self.preview_rect(session.payload(), current);

        // A landing indicator at the resolved position: an insertion line where the
        // card would go, which is the feedback that makes the drop predictable.
        if let Some(column_rect) = preview {
            let insert_index = self.insert_index_at(self.hovered_column.unwrap_or(0), current);
            let line_y = column_rect.y
                + HEADER_HEIGHT as i32
                + (insert_index as i32) * (CARD_HEIGHT as i32 + CARD_GAP)
                - CARD_GAP / 2;
            context.draw_line_stroke(
                Point::new(column_rect.x + 6, line_y),
                Point::new(column_rect.x + column_rect.width as i32 - 6, line_y),
                chrome.insertion(),
                3,
            );
        }

        // The card itself, centred on the pointer and semi-transparent so the board
        // underneath stays readable. It is clamped to the board, because a ghost centred on a
        // pointer near the edge would otherwise paint over whatever the layout placed beside
        // the board — nothing clips a widget at this layer.
        let label = session.payload().label.as_str();
        let board = self.geometry();
        let ghost_column =
            self.hovered_column.filter(|index| *index < self.columns.len()).unwrap_or(0);
        let ghost_width = self.column_width_at(ghost_column);
        let ghost_x = (current.x - (ghost_width as i32 / 2))
            .clamp(board.x, board.x + board.width.saturating_sub(ghost_width) as i32);
        let ghost_y = (current.y - (CARD_HEIGHT as i32 / 2))
            .clamp(board.y, board.y + board.height.saturating_sub(CARD_HEIGHT) as i32);
        let ghost = Rect::new(ghost_x, ghost_y, ghost_width, CARD_HEIGHT);
        context.fill_rounded_rect(ghost, 6, chrome.ghost().with_alpha(230));
        context.draw_rounded_rect_stroke(ghost, 6, chrome.insertion(), 2);
        if !label.is_empty() {
            let ghost_font = role_font(self.base.style(), 12.0);
            let ghost_line = context.text_line(ghost, &ghost_font);
            context.draw_text(
                Point::new(ghost.x + CARD_PADDING, ghost_line.y),
                label,
                &ghost_font,
                chrome.text(),
                HorizontalAlignment::Left,
            );
        }
    }
}
