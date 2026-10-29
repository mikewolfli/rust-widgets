// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

// The two test modules that used to live inline in `kanban_board.rs` move here unchanged;
// each keeps its own `use super::*` and therefore sees the board's imports through this file.

#[cfg(test)]
// Named `cases`, not `tests`: this file is already the `tests` module, and a nested module of
// the same name trips clippy::module_inception.
mod cases {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    // The activation tests observe delivery through a shared sink, so they need the
    // same Arc/Mutex the signal machinery uses.
    use crate::compat::{Arc, Mutex};

    /// A board with three columns and one card in the first.
    fn board() -> KanbanBoard {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_column(KanbanColumn::new("doing", "Doing"));
        board.add_column(KanbanColumn::new("done", "Done"));
        board.add_card(0, KanbanCard::new("a", "First"));
        board
    }

    /// Renders the board and returns the RGBA frame.
    fn render(board: &mut KanbanBoard, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The centre of the card at `position`.
    fn card_center(board: &KanbanBoard, position: CardPosition) -> Point {
        let rect = board.card_rect(position).expect("card is laid out");
        Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2)
    }

    #[test]
    fn kanban_creation_defaults() {
        let board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        assert_eq!(board.kind(), WidgetKind::KanbanBoard);
        assert_eq!(board.column_count(), 0);
        assert!(board.cards().is_empty());
        assert!(!board.is_dragging_card());
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_add_and_remove_columns() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        board.add_column(KanbanColumn::new("a", "A"));
        board.add_column(KanbanColumn::new("b", "B"));
        assert_eq!(board.column_count(), 2);
        assert_eq!(board.column(0).map(|c| c.title.as_str()), Some("A"));

        let removed = board.remove_column(0).expect("column 0");
        assert_eq!(removed.id, "a");
        assert_eq!(board.column_count(), 1);
        assert!(board.remove_column(9).is_none());
    }

    #[test]
    fn kanban_add_card_ignores_wip_limit() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        board.add_column(KanbanColumn::new("wip", "WIP").with_wip_limit(1));
        // Seeding is not an interactive drag; the limit is a display constraint on
        // drags, not a hard invariant.
        assert!(board.add_card(0, KanbanCard::new("a", "A")).is_some());
        assert!(board.add_card(0, KanbanCard::new("b", "B")).is_some());
        assert_eq!(board.column(0).unwrap().cards.len(), 2);
        assert!(board.column(0).unwrap().is_at_wip_limit());
    }

    #[test]
    fn kanban_add_card_rejects_unknown_column() {
        let mut board = board();
        assert!(board.add_card(9, KanbanCard::new("x", "X")).is_none());
    }

    #[test]
    fn kanban_position_of_finds_a_card_anywhere() {
        let mut board = board();
        board.add_card(1, KanbanCard::new("b", "Second"));
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
        assert_eq!(board.position_of("b"), Some(CardPosition { column: 1, card: 0 }));
        assert_eq!(board.position_of("missing"), None);
    }

    #[test]
    fn kanban_move_card_across_columns() {
        let mut board = board();
        let moved = board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 1, card: 0 });
        assert_eq!(moved, Some(CardPosition { column: 1, card: 0 }));
        assert!(board.column(0).unwrap().cards.is_empty());
        assert_eq!(board.column(1).unwrap().cards[0].id, "a");
    }

    #[test]
    fn kanban_move_card_within_a_column() {
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_card(0, KanbanCard::new("c", "Third"));

        // Move the first card to the end.
        let moved = board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 0, card: 2 });
        assert!(moved.is_some());
        let ids: Vec<&str> = board.column(0).unwrap().cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn kanban_move_card_down_by_one_lands_one_step_lower() {
        // The removal-first rule is what makes this true. Computing the target index
        // against the pre-removal list would leave the card where it started.
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_card(0, KanbanCard::new("c", "Third"));

        board.move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 0, card: 1 });
        let ids: Vec<&str> = board.column(0).unwrap().cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"], "a moved down exactly one place");
    }

    #[test]
    fn kanban_move_card_rejects_bad_indices() {
        let mut board = board();
        // Target column does not exist.
        assert!(board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 9, card: 0 })
            .is_none());
        // Source card does not exist.
        assert!(board
            .move_card(CardPosition { column: 0, card: 9 }, CardPosition { column: 1, card: 0 })
            .is_none());
        // Nothing was lost.
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    #[test]
    fn kanban_move_card_emits_the_signal() {
        let mut board = board();
        let moved =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, usize, usize)>::new()));
        let sink = moved.clone();
        board.card_moved.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((value.0.clone(), value.1.column, value.1.card));
            }
        });

        board.move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 2, card: 0 });
        assert_eq!(*moved.lock().expect("signal lock poisoned"), vec![("a".to_string(), 2, 0)]);
    }

    #[test]
    fn kanban_remove_card_returns_it_with_its_position() {
        let mut board = board();
        let (card, position) = board.remove_card("a").expect("card a");
        assert_eq!(card.title, "First");
        assert_eq!(position, CardPosition { column: 0, card: 0 });
        assert!(board.remove_card("a").is_none());
    }

    // ── Drag and drop (the `event::dnd` integration) ─────────────────────────

    #[test]
    fn kanban_press_on_a_card_opens_a_drag() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        assert_eq!(board.dragged_card_id(), Some("a"));
        // Not yet active: a press alone is a candidate click.
        assert!(!board.is_dragging_card());
    }

    #[test]
    fn card_view_toggles_done_and_reports_a_committed_edit() {
        let mut card = KanbanCardView::new(Rect::new(0, 0, 220, 72), "Write spec");
        card.set_description("Owner: Ada");
        card.set_tags(vec!["ui".to_string(), "p1".to_string()]);

        let done = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = Arc::clone(&done);
        card.card_done_toggled.connect(move |flag| {
            sink.lock().expect("done sink").push(*flag);
        });
        let clicked = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&clicked);
        card.card_clicked.connect(move || {
            *sink.lock().expect("click sink") += 1;
        });
        let edited = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&edited);
        card.card_edited.connect(move |title| {
            sink.lock().expect("edit sink").push(title.as_str().to_string());
        });

        let box_rect = card.done_box_rect();
        card.handle_event(&Event::mouse_press(box_rect.x + 2, box_rect.y + 2, 1));
        assert!(card.is_done());
        assert_eq!(*done.lock().expect("done sink"), vec![true]);

        card.handle_event(&Event::mouse_press(100, 50, 1));
        assert_eq!(*clicked.lock().expect("click sink"), 1);

        assert!(card.begin_title_edit());
        card.handle_event(&Event::text_input("!"));
        card.handle_event(&Event::key_press(13, 0));
        assert_eq!(card.title(), "Write spec!");
        assert_eq!(edited.lock().expect("edit sink").as_slice(), ["Write spec!".to_string()]);
    }

    #[test]
    fn card_view_properties_round_trip() {
        let mut card = KanbanCardView::new(Rect::new(0, 0, 220, 72), "Card");
        assert_eq!(card.get("title"), Ok(CapabilityValue::String("Card".to_string())));
        assert!(card.set("done", CapabilityValue::Bool(true)).is_ok());
        assert!(card.is_done());
        assert!(card
            .set("tags", CapabilityValue::Tuple(vec![CapabilityValue::String("a".to_string())]))
            .is_ok());
        assert_eq!(card.tags(), ["a".to_string()]);
        assert_eq!(
            card.set("editing", CapabilityValue::Bool(false)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn column_header_reports_rename_collapse_and_add() {
        let mut header = KanbanColumnHeader::new(Rect::new(0, 0, 220, 32), "Todo");
        header.set_count(2);
        header.set_wip_limit(Some(2));
        assert!(header.is_at_wip_limit());

        let renamed = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&renamed);
        header.column_renamed.connect(move |title| {
            sink.lock().expect("rename sink").push(title.as_str().to_string());
        });
        let collapsed = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = Arc::clone(&collapsed);
        header.column_collapsed_changed.connect(move |flag| {
            sink.lock().expect("collapse sink").push(*flag);
        });
        let added = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&added);
        header.add_card_requested.connect(move || {
            *sink.lock().expect("add sink") += 1;
        });

        let add = header.add_rect();
        header.handle_event(&Event::mouse_press(add.x + 2, add.y + 2, 1));
        assert_eq!(*added.lock().expect("add sink"), 1);

        let collapse = header.collapse_rect();
        header.handle_event(&Event::mouse_press(collapse.x + 2, collapse.y + 2, 1));
        assert!(header.is_collapsed());
        assert_eq!(*collapsed.lock().expect("collapse sink"), vec![true]);

        assert!(header.rename("In Progress"));
        assert!(!header.rename("In Progress"), "a no-op rename is not reported");
        assert_eq!(renamed.lock().expect("rename sink").as_slice(), ["In Progress".to_string()]);
        assert_eq!(header.get("wip_limit"), Ok(CapabilityValue::UInt(2)));
    }

    #[test]
    fn column_header_edits_its_title_when_editable() {
        let mut header = KanbanColumnHeader::new(Rect::new(0, 0, 220, 32), "Todo");
        assert!(header.begin_title_edit());
        header.handle_event(&Event::text_input("s"));
        header.handle_event(&Event::key_press(13, 0));
        assert_eq!(header.title(), "Todos");

        header.set_editable(false);
        assert!(!header.begin_title_edit(), "a read-only header cannot be renamed by the reader");
    }

    #[test]
    fn kanban_press_away_from_a_card_opens_no_drag() {
        let mut board = board();
        // Below the only card, still inside the column.
        board.handle_event(&Event::mouse_press(100, 300, 1));
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_drag_below_threshold_stays_a_click() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        board.handle_event(&Event::mouse_move(centre.x + 1, centre.y));
        assert!(!board.is_dragging_card());
    }

    /// A sub-threshold press/release is a click, and a click must be observable.
    ///
    /// Before this was fixed the board produced `card_activated` from **nowhere**:
    /// the press opened a drag session, the release discarded it, and the documented
    /// signal never fired from a real pointer. The assertion is deliberately on
    /// *delivery* (the id arriving), not on the gesture merely being non-dragging, so
    /// it fails if the emit is removed again.
    #[test]
    fn kanban_click_without_drag_emits_card_activated() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        let seen = Arc::new(Mutex::new(Option::<String>::None));
        let sink = Arc::clone(&seen);
        board.card_activated.connect(move |id: Arc<String>| {
            *sink.lock().expect("activation sink poisoned") = Some(id.as_str().to_string());
        });

        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        board.handle_event(&Event::mouse_move(centre.x + 1, centre.y));
        board.handle_event(&Event::mouse_release(centre.x + 1, centre.y, 1));

        assert_eq!(
            seen.lock().expect("activation sink poisoned").as_deref(),
            Some("a"),
            "a click on card 'a' must emit card_activated with its id"
        );
        // A click is not a move: the card stays where it was.
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    /// A real drag must **not** report an activation, or a drop would look like a click.
    #[test]
    fn kanban_drag_does_not_emit_card_activated() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let activations = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&activations);
        board.card_activated.connect(move |_id: Arc<String>| {
            *counter.lock().expect("activation counter poisoned") += 1;
        });

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 10));
        board.handle_event(&Event::mouse_release(start.x + 40, start.y + 10, 1));

        assert_eq!(
            *activations.lock().expect("activation counter poisoned"),
            0,
            "a drag that moved the card must not also report an activation"
        );
    }

    #[test]
    fn kanban_drag_across_columns_moves_the_card() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let target_column = board.column_rect(2).expect("column 2");
        let target = Point::new(target_column.x + 100, target_column.y + HEADER_HEIGHT as i32 + 20);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 10));
        assert!(board.is_dragging_card(), "the drag must be active before the drop");
        board.handle_event(&Event::mouse_move(target.x, target.y));
        board.handle_event(&Event::mouse_release(target.x, target.y, 1));

        assert_eq!(board.position_of("a"), Some(CardPosition { column: 2, card: 0 }));
        assert!(board.column(0).unwrap().cards.is_empty());
    }

    #[test]
    fn kanban_drop_outside_any_column_leaves_the_card_alone() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        // Far to the right of the last column.
        board.handle_event(&Event::mouse_release(5000, 200, 1));

        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
        assert!(!board.is_dragging_card());
    }

    #[test]
    fn kanban_wip_limit_refuses_a_card_from_another_column() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_column(KanbanColumn::new("full", "Full").with_wip_limit(1));
        board.add_card(0, KanbanCard::new("dragged", "Dragged"));
        // Fill the limited column to its cap.
        board.add_card(1, KanbanCard::new("resident", "Resident"));

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let target_column = board.column_rect(1).expect("column 1");
        let target = Point::new(target_column.x + 100, target_column.y + HEADER_HEIGHT as i32 + 20);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        board.handle_event(&Event::mouse_move(target.x, target.y));
        board.handle_event(&Event::mouse_release(target.x, target.y, 1));

        assert_eq!(
            board.position_of("dragged"),
            Some(CardPosition { column: 0, card: 0 }),
            "a full column must refuse an incoming card"
        );
        assert_eq!(board.column(1).unwrap().cards.len(), 1);
    }

    #[test]
    fn kanban_wip_limit_allows_reordering_within_the_full_column() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("full", "Full").with_wip_limit(1));
        board.add_card(0, KanbanCard::new("only", "Only"));

        // A limit caps *arrivals*, not movement. `can_accept` is the
        // position-independent question and a full board answers `false` to it — the
        // destination's own limit is what `on_drop` consults, where the column is
        // known. Asserting both halves pins which method owns which rule.
        let payload = DragPayload::new(CARD_PAYLOAD_TYPE, "only");
        assert!(
            !board.can_accept(&payload),
            "a board whose only column is full has nowhere to put a card"
        );

        // And the full column still accepts a card that is already inside it.
        let column_rect = board.column_rect(0).expect("column");
        let inside = Point::new(column_rect.x + 10, column_rect.y + HEADER_HEIGHT as i32 + 10);
        board.drag_origin = Some(CardPosition { column: 0, card: 0 });
        assert_eq!(
            board.on_drop(&payload, inside),
            DropEffect::Move,
            "a card already in the full column may be reordered within it"
        );
    }

    #[test]
    fn kanban_rejects_a_payload_of_another_type() {
        let board = board();
        assert!(!board.can_accept(&DragPayload::new("file", "x")));
    }

    #[test]
    fn kanban_empty_board_accepts_nothing() {
        let board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        // Nowhere to put a card is a refusal, not a silent no-op.
        assert!(!board.can_accept(&DragPayload::new(CARD_PAYLOAD_TYPE, "x")));
    }

    #[test]
    fn kanban_preview_rect_is_the_hovered_column() {
        let board = board();
        let payload = DragPayload::new(CARD_PAYLOAD_TYPE, "a");
        let column = board.column_rect(1).expect("column 1");
        let inside = Point::new(column.x + 10, column.y + 10);
        assert_eq!(board.preview_rect(&payload, inside), Some(column));
        // Outside every column there is no landing zone.
        assert_eq!(board.preview_rect(&payload, Point::new(5000, 5000)), None);
    }

    #[test]
    fn kanban_cancel_drag_restores_the_state() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));

        board.cancel_drag();
        assert!(!board.is_dragging_card());
        assert_eq!(board.dragged_card_id(), None);
        assert_eq!(board.hovered_column(), None);
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    #[test]
    fn kanban_mouse_leave_cancels_the_drag() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        assert!(board.is_dragging_card());

        board.handle_event(&Event::MouseLeave { pos: start });
        assert!(!board.is_dragging_card(), "losing the pointer must not strand a drag");
    }

    #[test]
    fn kanban_disabled_ignores_drags() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.set_enabled(false);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_removing_a_dragged_card_column_cancels_the_drag() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("a", "A"));
        board.add_card(0, KanbanCard::new("card", "Card"));
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(board.dragged_card_id(), Some("card"));

        board.remove_column(0);
        assert_eq!(board.dragged_card_id(), None, "the dragged card's column is gone");
    }

    // ── Layout and drawing ──────────────────────────────────────────────────

    #[test]
    fn kanban_columns_lay_out_side_by_side() {
        let board = board();
        let first = board.column_rect(0).expect("column 0");
        let second = board.column_rect(1).expect("column 1");
        assert_eq!(second.x - first.x, DEFAULT_COLUMN_WIDTH as i32 + COLUMN_GAP);
        assert_eq!(first.width, DEFAULT_COLUMN_WIDTH);
        assert!(board.column_rect(9).is_none());
    }

    #[test]
    fn kanban_collapsed_column_lays_out_no_cards() {
        let mut board = board();
        assert!(board.card_rect(CardPosition { column: 0, card: 0 }).is_some());
        board.column_mut(0).unwrap().collapsed = true;
        assert_eq!(
            board.card_rect(CardPosition { column: 0, card: 0 }),
            None,
            "a collapsed column shows no cards"
        );
    }

    #[test]
    fn kanban_draw_paints_without_panicking() {
        let mut board = board();
        let rgba = render(&mut board, Size::new(800, 400));
        assert!(!rgba.is_empty());
        // The board background is a light grey, so a frame that painted something
        // must differ from pure white.
        let painted = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| !(px[0] == 255 && px[1] == 255 && px[2] == 255))
            .count();
        assert!(painted > 0, "the board must paint its columns");
    }

    #[test]
    fn kanban_draw_zero_geometry_does_not_panic() {
        let mut board = board();
        let rgba = render(&mut board, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn kanban_drag_overlay_differs_from_the_resting_frame() {
        let mut board = board();
        let resting = render(&mut board, Size::new(800, 400));

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 20));
        let dragging = render(&mut board, Size::new(800, 400));

        assert_ne!(resting, dragging, "a drag must be visible");
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn kanban_derived_properties_are_read_only() {
        let mut board = board();
        for name in ["column_count", "card_count", "dragging_card_id", "hovered_column"] {
            assert_eq!(
                board.set(name, CapabilityValue::UInt(3)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
        // `column_width` used to be in that list; SP-15 makes it the one display dimension the
        // caller owns, so it is writable and only the derived counts stay read-only.
        assert!(board.set("column_width", CapabilityValue::UInt(300)).is_ok());
        assert_eq!(board.column_width(), 300);
        assert_eq!(board.get("column_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(board.get("card_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(board.get("column_width").unwrap(), CapabilityValue::UInt(300));
    }

    #[test]
    fn kanban_drag_state_is_reported_through_properties() {
        let mut board = board();
        assert_eq!(board.get("dragging_card_id").unwrap(), CapabilityValue::Null);

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(
            board.get("dragging_card_id").unwrap(),
            CapabilityValue::String("a".to_string())
        );
        assert_eq!(board.get("hovered_column").unwrap(), CapabilityValue::UInt(0));
    }

    #[test]
    fn kanban_insert_card_clamps_an_over_large_index() {
        let mut board = board();
        let position = board.insert_card(0, 99, KanbanCard::new("z", "Z")).expect("insert");
        // Appended rather than refused, because "past the end" has one sensible
        // reading and the caller's intent is unambiguous.
        assert_eq!(position, CardPosition { column: 0, card: 1 });
        assert_eq!(board.column(0).unwrap().cards[1].id, "z");
    }

    #[test]
    fn kanban_insert_index_uses_the_midpoint_rule() {
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        let first = board.card_rect(CardPosition { column: 0, card: 0 }).expect("card 0");
        let column_x = first.x + 10;

        // Just into the first card: before it.
        assert_eq!(board.insert_index_at(0, Point::new(column_x, first.y + 2)), 0);
        // Past the first card's centre: after it.
        assert_eq!(
            board.insert_index_at(0, Point::new(column_x, first.y + CARD_HEIGHT as i32 - 2)),
            1
        );
        // A drop in the header goes to the top.
        let column_rect = board.column_rect(0).expect("column");
        assert_eq!(board.insert_index_at(0, Point::new(column_x, column_rect.y + 2)), 0);
        // Far below everything appends.
        assert_eq!(board.insert_index_at(0, Point::new(column_x, column_rect.y + 2000)), 2);
    }

    #[test]
    fn column_width_is_writable_and_moves_the_next_column() {
        let mut board = board();
        let before = board.column_rect(1).expect("column 1").x;
        board.set_column_width(320);
        assert_eq!(board.column_width(), 320);
        assert_eq!(board.get("column_width").unwrap(), CapabilityValue::UInt(320));
        let after = board.column_rect(1).expect("column 1").x;
        assert_eq!(after - before, 100, "a wider column must displace the one beside it");
        assert_eq!(board.column_rect(0).unwrap().width, 320);

        // A width below the floor is clamped rather than producing an unusable column.
        board.set("column_width", CapabilityValue::UInt(1)).expect("writable");
        assert_eq!(board.column_width(), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn horizontal_scroll_shifts_columns_and_clamps_to_content() {
        // Three 220 px columns plus gaps are narrower than this board, so the test widens them
        // first to give the content somewhere to scroll.
        let mut board = board();
        board.set_column_width(400);
        let unscrolled = board.column_rect(0).expect("column 0").x;
        board.set_horizontal_scroll(100);
        assert_eq!(board.horizontal_scroll(), 100);
        assert_eq!(board.column_rect(0).unwrap().x, unscrolled - 100);

        // Past the end is clamped to the content's own width, not left unbounded.
        board.set_horizontal_scroll(100_000);
        assert_eq!(board.horizontal_scroll(), board.max_horizontal_scroll());
        // Reset and a negative request both stop at zero.
        board.set_horizontal_scroll(-50);
        assert_eq!(board.horizontal_scroll(), 0);

        // Shift-wheel scrolls horizontally instead of scrolling a column's cards.
        board.handle_event(&Event::wheel(0, -120, 0x0001));
        assert!(board.horizontal_scroll() > 0);
    }

    #[test]
    fn horizontal_scroll_is_published_as_a_property() {
        let mut board = board();
        board.set_column_width(400);
        board.set("horizontal_scroll_offset", CapabilityValue::UInt(64)).expect("writable");
        assert_eq!(board.horizontal_scroll(), 64);
        assert_eq!(board.get("horizontal_scroll_offset").unwrap(), CapabilityValue::UInt(64));
    }

    fn lane_board() -> KanbanBoard {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 600, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_card(0, KanbanCard::new("a", "First"));
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_lane(KanbanLane::new("fast", "Fast"));
        board.add_lane(KanbanLane::new("slow", "Slow"));
        board.set_lane_mode(KanbanLaneMode::Lanes);
        board
    }

    /// The lane model: a board-wide card id maps to a lane id, and an unassigned card renders in
    /// the first lane rather than disappearing.
    #[test]
    fn lanes_assign_cards_and_drive_layout() {
        let mut board = lane_board();
        assert_eq!(board.lane_count(), 2);
        assert_eq!(board.lane("slow").map(|lane| lane.title.as_str()), Some("Slow"));
        assert!(board.set_card_lane("b", "slow"));
        assert_eq!(board.card_lane("b"), Some("slow"));
        assert_eq!(board.lane_of("b"), Some("slow"), "lane_of is the read alias");
        assert_eq!(board.card_lane("a"), None, "unassigned cards fall into lane 0");
        assert!(!board.set_card_lane("missing", "slow"));
        assert!(!board.set_card_lane("a", "missing"));
        assert!(board.set_lane("a", "slow"), "set_lane is the write alias");
        assert_eq!(board.lane_of("a"), Some("slow"));
        assert!(!board.set_lane("missing", "slow"));
        // Restore the fixture's layout expectation: `a` again renders in the first lane.
        assert!(board.set_lane("a", "fast"));

        let lane0 = board.lane_band_rect(0).expect("lane 0");
        let lane1 = board.lane_band_rect(1).expect("lane 1");
        assert_eq!(lane1.x, LANE_HEADER_WIDTH as i32, "bands start after the labels");
        assert!(lane1.y >= lane0.y + lane0.height as i32);
        assert_eq!(board.lane_header_rect(1).expect("header").x, 0);

        let a = board.card_rect(CardPosition { column: 0, card: 0 }).expect("a laid out");
        let b = board.card_rect(CardPosition { column: 0, card: 1 }).expect("b laid out");
        assert!(a.y >= lane0.y && a.y < lane1.y, "a sits in the first band");
        assert!(b.y >= lane1.y, "b sits in the second band");
    }

    /// Paint and hit agree about the lane: a card hit-tests inside its band, and a painted lane
    /// board produces a frame.
    #[test]
    fn lane_mode_paints_and_hit_tests_inside_the_band() {
        let mut board = lane_board();
        assert!(board.set_card_lane("b", "slow"));
        let frame = render(&mut board, Size::new(600, 400));
        assert!(!frame.is_empty());

        let b = board.card_rect(CardPosition { column: 0, card: 1 }).expect("b laid out");
        board.handle_event(&Event::mouse_move(b.x + 10, b.y + 10));
        assert_eq!(board.hovered_card(), Some(CardPosition { column: 0, card: 1 }));

        // The lane strip itself is not a card slot.
        board.handle_event(&Event::mouse_move(10, b.y + 10));
        assert_eq!(board.hovered_card(), None);
    }

    /// A release inside another band reassigns the card, so the model follows the drop.
    #[test]
    fn dropping_a_card_in_another_lane_reassigns_it() {
        let mut board = lane_board();
        assert!(board.set_card_lane("b", "slow"));
        let a = board.card_rect(CardPosition { column: 0, card: 0 }).expect("a laid out");
        let slow = board.lane_band_rect(1).expect("slow lane");
        let target_y = slow.y + LANE_HEADER_HEIGHT + 10;

        board.handle_event(&Event::mouse_press(a.x + 10, a.y + 10, 1));
        board.handle_event(&Event::mouse_move(a.x + 10, target_y));
        board.handle_event(&Event::mouse_release(a.x + 10, target_y, 1));
        assert_eq!(board.card_lane("a"), Some("slow"));
    }

    #[test]
    fn lane_mode_is_a_property() {
        let mut board = lane_board();
        assert_eq!(board.get("lane_mode").unwrap(), CapabilityValue::String("lanes".to_string()));
        assert_eq!(board.get("swimlanes").unwrap(), CapabilityValue::Bool(true));
        board.set("swimlanes", CapabilityValue::Bool(false)).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::None);
        board.set("swimlanes", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::Lanes);
        board.set("lane_mode", CapabilityValue::String("none".to_string())).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::None);
        assert!(board.set("lane_mode", CapabilityValue::String("diagonal".to_string())).is_err());
        assert!(board.set("swimlanes", CapabilityValue::UInt(1)).is_err());
        assert_eq!(
            board.set("lane_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// The two levels round-trip through their flat record lists: a fresh board fed the
    /// `columns` and `cards` a seeded board reports reads the same state back.
    #[test]
    fn columns_and_cards_round_trip_through_records() {
        let mut source = lane_board();
        assert!(source.set_card_lane("b", "slow"));
        let mut target = KanbanBoard::new(Rect::new(0, 0, 600, 400));
        target.add_lane(KanbanLane::new("fast", "Fast"));
        target.add_lane(KanbanLane::new("slow", "Slow"));

        target.set("columns", source.get("columns").unwrap()).expect("columns writable");
        target.set("cards", source.get("cards").unwrap()).expect("cards writable");

        assert_eq!(target.get("columns").unwrap(), source.get("columns").unwrap());
        assert_eq!(target.get("cards").unwrap(), source.get("cards").unwrap());
        assert_eq!(target.get("column_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(target.get("card_count").unwrap(), CapabilityValue::UInt(2));
        assert_eq!(target.lane_of("a"), None);
        assert_eq!(target.lane_of("b"), Some("slow"));
    }

    /// A card row naming a column the board does not have is refused **whole**: the
    /// board keeps the cards it had rather than accepting the rows that did resolve.
    #[test]
    fn a_card_naming_an_unknown_column_is_refused_without_side_effects() {
        let mut board = board();
        let CapabilityValue::Records(mut list) = board.get("cards").unwrap() else {
            panic!("cards must be records");
        };
        list = crate::widget::capability::RecordList::from_items(
            &KANBAN_CARD_SCHEMA,
            list.items()
                .iter()
                .map(|row| {
                    let mut row = row.clone();
                    row = row.with("column", CapabilityValue::UInt(99));
                    row
                })
                .collect(),
        );
        let refused = board.set("cards", CapabilityValue::Records(list));
        assert!(
            matches!(refused, Err(CapabilityAccessError::ConstraintViolation { .. })),
            "expected a constraint violation, got {refused:?}"
        );
        assert_eq!(board.get("column_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(board.get("card_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(board.columns()[0].cards[0].id, "a");
    }

    /// A `columns` write adopts the existing cards of a column with the same id, so
    /// renaming or reordering columns never loses cards.
    #[test]
    fn a_column_write_adopts_its_cards_by_id() {
        let mut board = board();
        board.set("columns", board.get("columns").unwrap()).expect("identity write");
        assert_eq!(board.column_count(), 3);
        assert_eq!(board.columns()[0].cards.len(), 1, "todo keeps its card");
        assert_eq!(board.columns()[0].cards[0].id, "a");
    }

    /// The list-valued commands are payload-free spellings of the property write, so a
    /// bare invocation reports that the value is what is missing.
    #[test]
    fn set_columns_and_set_cards_report_a_missing_payload() {
        let mut board = board();
        assert_eq!(board.command("set_columns"), Err(CapabilityAccessError::OutOfRange));
        assert_eq!(board.command("set_cards"), Err(CapabilityAccessError::OutOfRange));
    }
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    fn board() -> KanbanBoard {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 600, 300));
        board.add_column(KanbanColumn::new("todo", "To do"));
        board.add_card(0, KanbanCard::new("a", "Alpha"));
        board.add_card(0, KanbanCard::new("b", "Beta"));
        board.add_card(0, KanbanCard::new("c", "Gamma"));
        board
    }

    /// SP-07: the selection is a set, ordered by the board's own reading order.
    #[test]
    fn multi_selection_keeps_display_order_and_drops_removed_cards() {
        let mut board = board();
        assert!(board.set_card_selected("c", true));
        assert!(board.set_card_selected("a", true));
        assert_eq!(board.selected_cards(), ["a", "c"]);
        assert!(board.set_card_selected("a", false));
        assert_eq!(board.selected_cards(), ["c"]);
        board.remove_card("c");
        assert!(board.selected_cards().is_empty());
        assert!(!board.set_card_selected("missing", true));
    }

    /// SP-07: a modified press toggles a card instead of starting a drag.
    #[test]
    fn a_primary_modified_press_toggles_the_selection() {
        let mut board = board();
        let rect = board.card_rect(CardPosition { column: 0, card: 0 }).expect("a laid-out card");
        let primary = crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits();
        board.handle_event(&Event::mouse_press_with(rect.x + 4, rect.y + 4, 1, primary));
        assert_eq!(board.selected_cards(), ["a"]);
        assert!(!board.is_dragging_card());
        board.handle_event(&Event::mouse_press_with(rect.x + 4, rect.y + 4, 1, primary));
        assert!(board.selected_cards().is_empty());
    }

    /// SP-05: the hover signal carries the card id and a `None` on the way out.
    #[test]
    fn hovering_a_card_emits_its_id_and_leaving_emits_none() {
        let mut board = board();
        let seen = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let sink = seen.clone();
        board.card_hovered.connect_ref(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.clone());
            }
        });
        let rect = board.card_rect(CardPosition { column: 0, card: 1 }).expect("a laid-out card");
        board.handle_event(&Event::mouse_move(rect.x + 4, rect.y + 4));
        board.handle_event(&Event::mouse_leave(rect.x + 4, rect.y + 4));
        let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(seen, vec![Some("b".to_string()), None]);
    }

    /// SP-P11: the id index answers the same positions the linear scan used to.
    #[test]
    fn the_card_index_matches_every_card_position() {
        let board = board();
        for (position, card) in board.cards() {
            assert_eq!(board.position_of(&card.id), Some(position));
        }
        assert!(board.position_of("missing").is_none());
    }

    /// SP-010: the hit test resolves the slot arithmetically, including under a column scroll.
    #[test]
    fn card_hit_test_resolves_the_scrolled_slot() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("big", "Big"));
        for index in 0..200 {
            board.add_card(0, KanbanCard::new(format!("c{index}"), format!("C{index}")));
        }
        let rect = board.card_rect(CardPosition { column: 0, card: 5 }).expect("card 5");
        assert_eq!(
            board.card_at(Point::new(rect.x + 4, rect.y + 4)),
            Some(CardPosition { column: 0, card: 5 })
        );

        board.scroll_column(0, 10);
        let rect = board.card_rect(CardPosition { column: 0, card: 15 }).expect("card 15");
        assert_eq!(
            board.card_at(Point::new(rect.x + 4, rect.y + 4)),
            Some(CardPosition { column: 0, card: 15 }),
            "the pointer names the card in the model, not the slot on screen"
        );
        // The inter-card gap belongs to neither neighbour.
        assert_eq!(board.card_at(Point::new(rect.x + 4, rect.y + CARD_HEIGHT as i32 + 1)), None);
    }

    /// SP-010: a drop in a scrolled column names an index in the model.
    #[test]
    fn drop_index_accounts_for_column_scroll() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("big", "Big"));
        for index in 0..50 {
            board.add_card(0, KanbanCard::new(format!("c{index}"), format!("C{index}")));
        }
        board.scroll_column(0, 10);
        let column = board.column_rect(0).expect("column 0");
        let first_y = column.y + HEADER_HEIGHT as i32;
        assert_eq!(board.insert_index_at(0, Point::new(column.x + 4, first_y + 4)), 10);
    }

    /// SP-011: an override changes one column's width; the rest keep the global value.
    #[test]
    fn per_column_width_override_defaults_to_the_global_width() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_column(KanbanColumn::new("doing", "Doing"));
        assert_eq!(board.column_width_at(0), board.column_width());
        assert!(board.set_column_width_at(0, Some(300)));
        assert_eq!(board.column_width_at(0), 300);
        assert_eq!(board.column_width_at(1), board.column_width());

        let first = board.column_rect(0).expect("column 0");
        let second = board.column_rect(1).expect("column 1");
        assert_eq!(first.width, 300);
        assert_eq!(second.x - first.x, 300 + COLUMN_GAP);

        assert!(board.set_column_width_at(0, None));
        assert_eq!(board.column_width_at(0), board.column_width());
        assert_eq!(
            board.column_rect(1).unwrap().x - board.column_rect(0).unwrap().x,
            DEFAULT_COLUMN_WIDTH as i32 + COLUMN_GAP
        );
        assert!(!board.set_column_width_at(9, Some(300)), "no such column");
        assert!(board.set_column_width_at(0, Some(1)));
        assert_eq!(board.column_width_at(0), MIN_COLUMN_WIDTH, "floored on write");
    }

    /// SP-011: a removed column takes its override with it.
    #[test]
    fn removing_a_column_drops_its_width_override() {
        let mut board = board();
        assert!(board.set_column_width_at(0, Some(300)));
        board.remove_column(0);
        assert!(!board.column_widths.contains_key("todo"));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        assert_eq!(board.column_width_at(board.columns.len() - 1), board.column_width());
    }

    /// SP-012: the inline chevron toggles the column, and the board forwards the same state.
    #[test]
    fn the_inline_chevron_and_board_api_toggle_collapse() {
        let mut board = board();
        let chevron = board.column_collapse_rect(0).expect("column 0 has a chevron");
        board.handle_event(&Event::mouse_press(chevron.x + 2, chevron.y + 2, 1));
        assert_eq!(board.is_column_collapsed(0), Some(true));
        assert!(
            board.card_rect(CardPosition { column: 0, card: 0 }).is_none(),
            "a collapsed column lays out no cards"
        );

        assert!(board.set_column_collapsed(0, false));
        assert!(board.card_rect(CardPosition { column: 0, card: 0 }).is_some());
        assert!(board.toggle_column_collapsed(0));
        assert_eq!(board.is_column_collapsed(0), Some(true));
        assert_eq!(board.is_column_collapsed(9), None);
        assert!(!board.toggle_column_collapsed(9));
    }

    /// The attached proxy follows the board's existing drag session and hides when it ends.
    #[test]
    fn the_drag_ghost_follows_the_board_drag() {
        let mut board = board();
        board.attach_drag_ghost_layer(DragGhostLayer::new(Rect::new(0, 0, 200, 60)));
        assert!(board.is_drag_ghost_layer_attached());
        assert_eq!(
            board.drag_ghost_layer().expect("attached").drop_bounds(),
            Some(board.geometry()),
            "attach pushes the board as the drop bound"
        );
        assert!(!board.drag_ghost_layer().expect("attached").is_active());
        assert!(!board.poll_drag_ghost_layer(), "no drag is nothing to apply");

        let position = CardPosition { column: 0, card: 0 };
        let origin = board.card_rect(position).expect("card is laid out");
        let start =
            Point::new(origin.x + origin.width as i32 / 2, origin.y + origin.height as i32 / 2);
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 10));
        assert!(board.is_dragging_card());
        assert!(board.poll_drag_ghost_layer(), "the active drag reaches the proxy");
        {
            let layer = board.drag_ghost_layer().expect("attached");
            assert!(layer.is_active());
            assert_eq!(layer.title(), "a", "the card id is the proxy title");
            assert_eq!(layer.origin_rect(), origin);
            assert_eq!(layer.pointer(), Point::new(start.x + 40, start.y + 10));
        }
        assert!(!board.poll_drag_ghost_layer(), "an unchanged drag is not re-applied");

        board.cancel_drag();
        assert!(board.poll_drag_ghost_layer(), "the ended drag hides the proxy");
        assert!(!board.drag_ghost_layer().expect("attached").is_active());

        let layer = board.detach_drag_ghost_layer().expect("detached");
        assert!(!board.is_drag_ghost_layer_attached());
        assert!(!board.poll_drag_ghost_layer(), "a detached proxy is not polled");
        assert!(!layer.is_active());
    }

    /// SPX-X13: the constant-time selection set stays in step with the ordered list.
    #[test]
    fn the_selection_set_tracks_every_selection_mutation() {
        let mut board = board();
        assert!(board.set_card_selected("a", true));
        assert!(board.set_card_selected("c", true));
        assert_eq!(board.selected_cards(), ["a", "c"]);
        assert_eq!(board.selected_set.len(), 2);

        board.remove_card("a");
        assert_eq!(board.selected_cards(), ["c"]);
        assert!(!board.selected_set.contains("a"), "a removed card leaves the set");

        board.add_card(0, KanbanCard::new("b", "Beta"));
        assert!(board.toggle_card_selected("b"));
        assert!(board.selected_set.contains("b"));

        board.clear_selection();
        assert!(board.selected_cards().is_empty());
        assert!(board.selected_set.is_empty());
    }
}
