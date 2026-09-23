// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_input {
    () => {
        pub(crate) const SLIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("single_step", PropertyValueKind::Int, true, true),
            PropertySchema::new("page_step", PropertyValueKind::Int, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            // The reader's tokens. `tick_position_to_str` returns `none`/`above`/`below`/`both`
            // while this published Qt's `noticks`/`left`/`right`/`ticksbothsides`. The writer
            // parses both spellings, so only the read direction can reveal the mismatch — which
            // is exactly why it survived until a schema-versus-reader gate existed. A caller who
            // formatted the property, or matched it against a list built from this schema, would
            // never have found a match for what the control reports.
            PropertySchema::enumerated(
                "tick_position",
                true,
                true,
                &["none", "above", "below", "both"],
            ),
            PropertySchema::new("tick_interval", PropertyValueKind::Int, true, true),
            PropertySchema::new("tracking", PropertyValueKind::Bool, true, true),
            PropertySchema::new("slider_position", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const PROGRESS_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("text_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("inverted_appearance", PropertyValueKind::Bool, true, true),
            // The direction the *line* runs in. Distinct from `inverted_appearance`, which is a
            // decoration: direction says which end the minimum is at, and the two compose (an RTL
            // bar draws inverted *without* the property being set). Declared in both tables so a
            // caller can read it back as well as write it.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            PropertySchema::new("progress", PropertyValueKind::Float, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const SCROLL_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("single_step", PropertyValueKind::Int, true, true),
            PropertySchema::new("page_step", PropertyValueKind::Int, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            // Only the horizontal trough is a line of text direction: the vertical axis is the block
            // flow, which a right-to-left script does not reverse.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            PropertySchema::new("slider_size", PropertyValueKind::Float, true, false),
            PropertySchema::new("slider_position", PropertyValueKind::Float, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const LIST_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated(
                "selection_mode",
                true,
                true,
                &["none", "single", "multi", "extended"],
            ),
            PropertySchema::new("current_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("item_height", PropertyValueKind::Float, true, true),
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const SPIN_BOX_PROPERTIES: &[PropertySchema] = &[
            // `Number`, not `Int`: these four report `Int` while `decimals == 0` (the
            // default, so every existing caller is unaffected) and `Float` once decimals
            // are enabled. Declaring `Int` would be a lie in decimal mode, and declaring
            // `Float` would be one in integer mode. See `PropertyValueKind::Number`.
            PropertySchema::new("minimum", PropertyValueKind::Number, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Number, true, true),
            PropertySchema::new("value", PropertyValueKind::Number, true, true),
            PropertySchema::new("single_step", PropertyValueKind::Number, true, true),
            PropertySchema::new("decimals", PropertyValueKind::UInt, true, true),
            PropertySchema::new("prefix", PropertyValueKind::String, true, true),
            PropertySchema::new("suffix", PropertyValueKind::String, true, true),
            // The two forms a consumer would otherwise have to rebuild: the announcement string (unit
            // marks included) and the number alone. `value_text` is exactly what the painter puts in the
            // value's own box, so a snapshot assertion and the drawn glyphs name the same string.
            PropertySchema::new("display_text", PropertyValueKind::String, true, false),
            PropertySchema::new("value_text", PropertyValueKind::String, true, false),
            PropertySchema::new("special_value_text", PropertyValueKind::String, true, true),
            PropertySchema::new("wrapping", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("current_text", PropertyValueKind::String, true, true),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("max_visible_items", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIAL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("single_step", PropertyValueKind::Int, true, true),
            PropertySchema::new("page_step", PropertyValueKind::Int, true, true),
            PropertySchema::new("notches_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("notch_target", PropertyValueKind::Float, true, true),
            PropertySchema::new("wrapping", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COMMAND_LINK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("description", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("current_font_family", PropertyValueKind::String, true, false),
            PropertySchema::new("item_count", PropertyValueKind::Int, true, false),
            PropertySchema::new("current_index", PropertyValueKind::Int, true, true),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("max_visible_items", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const LINE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder_text", PropertyValueKind::String, true, true),
            PropertySchema::new("max_length", PropertyValueKind::UInt, true, true),
            PropertySchema::new("read_only", PropertyValueKind::Bool, true, true),
            PropertySchema::new("cursor_position", PropertyValueKind::UInt, true, true),
            // The five non-value strings a text entry shows. `prefix`/`suffix` are in-field unit marks;
            // `helper`/`error`/`counter` share the support row below.
            PropertySchema::new("prefix", PropertyValueKind::String, true, true),
            PropertySchema::new("suffix", PropertyValueKind::String, true, true),
            PropertySchema::new("helper", PropertyValueKind::String, true, true),
            PropertySchema::new("error", PropertyValueKind::String, true, true),
            // Derived from the value's length and `max_length`, so readable but not writable.
            PropertySchema::new("counter", PropertyValueKind::String, true, false),
            PropertySchema::new("over_limit", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TEXT_EDIT_PROPERTIES: &[PropertySchema] = &[
            // These five are genuinely readable and writable: `TextEdit` routes
            // them to its own `set_text` / `set_placeholder_text` /
            // `set_max_length` / `set_read_only` / `set_line_wrap`. They were
            // declared `false, false`, which made the property route refuse
            // names the control publishes.
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder_text", PropertyValueKind::String, true, true),
            PropertySchema::new("max_length", PropertyValueKind::UInt, true, true),
            PropertySchema::new("read_only", PropertyValueKind::Bool, true, true),
            PropertySchema::new("line_wrap", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const SPINNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("active", PropertyValueKind::Bool, true, true),
            PropertySchema::new("thickness", PropertyValueKind::UInt, true, true),
            PropertySchema::new("speed", PropertyValueKind::Float, true, true),
            PropertySchema::new("size_ratio", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const ROLLER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("visible_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const DROPDOWN_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const TEXT_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("read_only", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const KEYBOARD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("layout", true, true, &["qwerty", "numeric"]),
            PropertySchema::new("lowercase", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `MarkdownEditor`'s own names are the document state and its derived
        // statistics. `preview_mode` is writable because `set_preview_mode` is a
        // reversible assignment; `cursor_line` is not, because the editor owns
        // where the caret sits in response to edits.
        pub(crate) const MARKDOWN_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("preview_mode", PropertyValueKind::Bool, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("word_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("heading_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("cursor_line", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const SWITCH_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEARCH_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAG_INPUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("tags", PropertyValueKind::String, true, false),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const IME_PREEDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("cursor_position", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const INPLACE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("editing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_SLIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            PropertySchema::new("min", PropertyValueKind::Float, true, true),
            PropertySchema::new("max", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MASKED_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("mask", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AUTO_COMPLETE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // The count describes the list; these describe the control. A consumer that could only read
            // the count could not tell whether the dropdown was open or which row was highlighted.
            PropertySchema::new("suggestion_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_suggestion", PropertyValueKind::String, true, false),
            PropertySchema::new("dropdown_visible", PropertyValueKind::Bool, true, false),
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::new("can_undo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_redo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MULTI_SELECT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RANGE_SLIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("min_value", PropertyValueKind::Float, true, true),
            PropertySchema::new("max_value", PropertyValueKind::Float, true, true),
            PropertySchema::new("lower", PropertyValueKind::Float, true, true),
            PropertySchema::new("upper", PropertyValueKind::Float, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            // Which end of the track the minimum sits at. `set_orientation` and `set_direction`
            // were both absent from this table even though the contract answers them, which is the
            // "control and schema disagree about which properties exist" defect the gate looks for.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FLOATING_LABEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // The caption that floats above the field, distinct from `text` (the content
            // the user types). Declared because the control answers it; the schema and the
            // contract disagreeing about which properties exist is itself the defect this
            // table is checked for.
            PropertySchema::new("label", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("focused", PropertyValueKind::Bool, true, true),
            // The tokens are exactly what `get` returns and exactly what `set` parses;
            // `published_enum_tokens_are_accepted_by_their_control` writes every one of
            // them back, so a drift between this list and the parser fails rather than
            // advertising a value a caller cannot write.
            PropertySchema::enumerated(
                "floating_label_behavior",
                true,
                true,
                &["auto", "always", "never"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_DATE_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_date", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EDITABLE_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RICH_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("read_only", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEARCH_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SHORTCUT_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("filter_text", PropertyValueKind::String, true, true),
            // `filter_text` is what the caller typed; these describe what it did. A consumer could set a
            // filter and had no way to ask how much of the table survived it.
            PropertySchema::new("shortcut_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("visible_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("category_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("categories", PropertyValueKind::String, true, false),
            PropertySchema::new("empty", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_undo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_redo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
