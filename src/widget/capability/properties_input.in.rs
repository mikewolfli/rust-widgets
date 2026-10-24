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
            // while this published the `noticks`/`left`/`right`/`ticksbothsides` spellings. The writer
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
            // The mode switch was fully implemented (`is_indeterminate` / `set_indeterminate`, and
            // the paint path's sweeping band) but unpublished, so a designer could never enable it.
            PropertySchema::new("indeterminate", PropertyValueKind::Bool, true, true),
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
            // The selected rows as a comma-joined list of indices, in visual order.
            // `selected_count` answers only *how many* rows are chosen; a multi-select list whose
            // contract cannot say *which* ones leaves a host unable to read back the selection it
            // just made. Comma-joined is the same spelling `tag_input`'s `tags` and the table's
            // sort/filter properties use, until an array variant exists.
            PropertySchema::new("selected_indices", PropertyValueKind::String, true, false),
            // The row the pointer is over, or `null`. Read-only: hover is the pointer's fact, not
            // a host's to set. Published so a host can drive an affordance from it.
            PropertySchema::new("hovered_row", PropertyValueKind::UInt, true, false),
            // The size the rows are drawn at, which auto-scales with `item_height`. Read-only and
            // derived: it reports the number the draw actually uses rather than a second knob that
            // could disagree with it.
            PropertySchema::new("font_size", PropertyValueKind::Float, true, false),
            // The rows' horizontal alignment. Only the three horizontal tokens are accepted: a row's
            // label is centred vertically in its band by the row's own layout, so a `top`/`bottom`
            // write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // The direction the field runs in. It moves the step column to the leading edge and
            // anchors the value to that same edge, so a caller in an RTL locale gets a spin box
            // whose number sits beside the buttons that change it. Declared in both directions so
            // it can be read back as well as written.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            // The value's horizontal alignment. Only the three horizontal tokens are accepted: the
            // value is centred vertically in the row band by the row's own layout, so a `top`/`bottom`
            // write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            // What `insert_item` does with a new value, and how the size hint reacts to the
            // item list. Both are token sets the control itself parses.
            PropertySchema::enumerated(
                "insert_policy",
                true,
                true,
                &[
                    "no_insert",
                    "insert_at_top",
                    "insert_at_current",
                    "insert_at_bottom",
                    "insert_alphabetically",
                ],
            ),
            PropertySchema::enumerated(
                "size_adjust_policy",
                true,
                true,
                &[
                    "adjust_to_minimum_contents_length",
                    "adjust_to_contents",
                    "adjust_to_contents_on_first_show",
                ],
            ),
            // The entry list is a *property*, not only the `set_items` command (whose signature takes
            // the items as an argument the property route could not carry). Without it a document's
            // `"items": [...]` had nowhere to go and a designer could not see or edit the entries at
            // all. It travels as `StringList` rather than `Records`: two entries may share a label,
            // so there is no identity field to key them by and `Records`' unique-id rule would
            // refuse a legitimate list.
            PropertySchema::new("items", PropertyValueKind::StringList, true, true),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("current_text", PropertyValueKind::String, true, true),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("max_visible_items", PropertyValueKind::UInt, true, true),
            // Whether the drop-down list is showing. Published so a host can read the state its
            // `popup_visibility_changed` event describes, and a designer can drive it.
            PropertySchema::new("open", PropertyValueKind::Bool, true, true),
            // The writing direction the field runs in. Published because it moves the indicator to
            // the other edge — a fact a host and a designer panel have to be able to read and set
            // rather than infer from the drawing.
            PropertySchema::new("direction", PropertyValueKind::String, true, true),
            // The value's horizontal alignment. Only the three horizontal tokens are accepted: the
            // value is centred vertically in the field band by the field's own layout, so a
            // `top`/`bottom` write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // An optional leading shield slot; opt-in so the default drawing is unchanged.
            PropertySchema::new("shield", PropertyValueKind::Bool, true, true),
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
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("max_visible_items", PropertyValueKind::Int, true, true),
            // The filter set and the optional preview text a font picker offers. The filter is a
            // token set (`monospaced`, `proportional`, `scalable`); the other two are nullable.
            PropertySchema::new("font_filters", PropertyValueKind::String, true, true),
            PropertySchema::new("writing_system_filter", PropertyValueKind::String, true, true),
            PropertySchema::new("sample_text", PropertyValueKind::String, true, true),
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
            // How the field echoes its value. Published because the *masking* is the point of
            // `Password`/`NoEcho`: the copy and cut paths consult it (so a masked field copies
            // its mask rather than the plaintext), and a host that can set the text but not the
            // echo mode cannot put a field into the one state that protects a credential.
            PropertySchema::enumerated("echo_mode", true, true, &["normal", "password", "no_echo"]),
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
            // Derived from the value and the field's grammar (see `InputValidator`): readable
            // so a form can drive its own submit state, not writable for the same reason the
            // counter is not — the text already determines both.
            PropertySchema::new("valid", PropertyValueKind::Bool, true, false),
            PropertySchema::new("validation_message", PropertyValueKind::String, true, false),
            // The cached outcome of the installed rule: `invalid`, `intermediate` or `acceptable`.
            PropertySchema::new("validation_state", PropertyValueKind::String, true, false),
            // The value's horizontal alignment. Only the three horizontal tokens are accepted: the
            // value is centred vertically in the field band by the field's own layout, so a
            // `top`/`bottom` write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            // The dirty flag, the clear affordance and the password reveal-on-edit switch are
            // all assigned through a setter the control owns, so both directions are served.
            PropertySchema::new("modified", PropertyValueKind::Bool, true, true),
            PropertySchema::new("clear_button_enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("password_echo_on_edit", PropertyValueKind::Bool, true, true),
            // Signed because the field's text can be scrolled back past the leading edge while
            // the caret is parked; the writer clamps to the same range the reader reports.
            PropertySchema::new("scroll_x", PropertyValueKind::Int, true, true),
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
            // Each line's horizontal alignment. Only the three horizontal tokens are accepted: the
            // document is laid out as rows down the interior, so a `top`/`bottom` write would be one
            // the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // The option list is a property as well as a command, for the same reason as
            // `combo_box.items`: a document has to be able to spell the wheel's entries. Entries
            // may repeat, so it is a `StringList` rather than `Records`.
            PropertySchema::new("options", PropertyValueKind::StringList, true, true),
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
            // A nullable row cap and the scroll position of the capped window.
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_offset", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const TEXT_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("read_only", PropertyValueKind::Bool, true, true),
            // Enforced by `set_max_length`, but unpublished: a designer had no way to see or set
            // the control's byte budget. `0` means unlimited, as on `line_edit`.
            PropertySchema::new("max_length", PropertyValueKind::UInt, true, true),
            // Wrapping is a document setting the area owns; the margin and the block budget are
            // both assigned through setters, and `max_block_count` accepts `Null` to mean
            // unlimited.
            PropertySchema::new("soft_wrap", PropertyValueKind::Bool, true, true),
            PropertySchema::new("content_margin", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_block_count", PropertyValueKind::UInt, true, true),
            // Derived from the caret's position within the laid-out document, so the area
            // answers it and refuses a write that would leave the caret behind.
            PropertySchema::new("scroll_x", PropertyValueKind::Int, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const KEYBOARD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("layout", true, true, &["qwerty", "numeric"]),
            PropertySchema::new("lowercase", PropertyValueKind::Bool, true, true),
            // Whether a key paints its pressed state before the character arrives.
            PropertySchema::new("press_animation", PropertyValueKind::Bool, true, true),
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
            // The value's horizontal alignment. Only the three horizontal tokens are accepted: the
            // value is centred vertically in the field band by the field's own layout, so a
            // `top`/`bottom` write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAG_INPUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("tags", PropertyValueKind::String, true, false),
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // A comma-joined set of trigger names (`double_click`, `single_click`, `f2`, or
            // `none`), so it is free text rather than a single-choice enum.
            PropertySchema::new("edit_triggers", PropertyValueKind::String, true, true),
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
            // The field's content horizontal alignment. Only the three horizontal tokens are accepted:
            // the characters are centred vertically in the field band by the field's own layout, so a
            // `top`/`bottom` write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("can_undo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_redo", PropertyValueKind::Bool, true, false),
            // How a suggestion is matched, whether the match is case sensitive, whether the
            // remainder is shown inline, and when the popup closes.
            PropertySchema::enumerated(
                "match_mode",
                true,
                true,
                &["contains", "starts_with", "ends_with", "fixed_string", "regexp"],
            ),
            PropertySchema::new("case_sensitive", PropertyValueKind::Bool, true, true),
            PropertySchema::new("inline_completion", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "close_policy",
                true,
                true,
                &["always", "never", "on_commit"],
            ),
            // The word being completed is derived from the value, so it is read-only.
            PropertySchema::new("completion_prefix", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MULTI_SELECT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // The field's caret, the popup's row cap (nullable) and its scroll position.
            PropertySchema::new("cursor_position", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_visible_items", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_offset", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
            // The value's horizontal alignment. Only the three horizontal tokens are accepted: the
            // value is centred vertically in the field band by the field's own layout, so a
            // `top`/`bottom` write would be one the control could never honour.
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
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
