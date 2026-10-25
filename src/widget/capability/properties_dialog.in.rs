// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_dialog {
    () => {
        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("hex_rgba", PropertyValueKind::String, true, true),
            PropertySchema::new("show_alpha", PropertyValueKind::Bool, true, true),
            PropertySchema::new("preset_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("presets", PropertyValueKind::StringList, true, true),
            PropertySchema::new("hue_degrees", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MESSAGE_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // The severity glyph. A standard message box carries one for
            // warning/error prompts, and this control draws it — the JSON loader
            // accepted an `icon` key all along, so the property layer and the
            // capability table were the halves that were missing.
            PropertySchema::enumerated(
                "icon",
                true,
                true,
                &["none", "information", "question", "warning", "critical"],
            ),
            // `true, true`: the control has both a getter and a setter (`is_modal` /
            // `set_modal`), so declaring it non-readable made the designer hide a property
            // that works. BLUE20 layer 2's Q1 gate found this by comparing the schema
            // against the control's own `property_names()`.
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            // The supplementary body text and the optional checkbox the box paints. All four
            // are read and written through the box's own accessors.
            PropertySchema::new("informative_text", PropertyValueKind::String, true, true),
            PropertySchema::new("detailed_text", PropertyValueKind::String, true, true),
            PropertySchema::new("check_box", PropertyValueKind::String, true, true),
            PropertySchema::new("check_box_checked", PropertyValueKind::Bool, true, true),
            // A standard-button token (for example `ok` or `cancel`), or `Null` for none; the
            // writer accepts exactly the spellings the reader produces.
            PropertySchema::new("default_button", PropertyValueKind::String, true, true),
            PropertySchema::new("escape_button", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FILE_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("directory", PropertyValueKind::String, true, true),
            // `true, true`: the host owns the list the dialog selects from, so it must be able to
            // write the row the user chose back into a *mounted* dialog by id. See
            // `FileDialog`'s `set` arm for why a read-only route made that impossible.
            PropertySchema::new("selected_file", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "mode",
                true,
                true,
                &["open_file", "open_files", "save_file", "select_directory"],
            ),
            // The prompt above the list and the suffix appended to a typed name. Both are
            // saved positions the host may write; the entry count and the two history flags
            // are derived from the dialog's own state and so are read-only.
            PropertySchema::new("label_text", PropertyValueKind::String, true, true),
            PropertySchema::new("default_suffix", PropertyValueKind::String, true, true),
            PropertySchema::new("file_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("can_go_back", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_go_forward", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("view_mode", true, true, &["list", "details"]),
            PropertySchema::new("sidebar_urls", PropertyValueKind::StringList, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            // The face options the dialog edits and the two display switches. Each is read and
            // written through the dialog's selected-font state or its option bitset.
            PropertySchema::new("underline", PropertyValueKind::Bool, true, true),
            PropertySchema::new("strikethrough", PropertyValueKind::Bool, true, true),
            PropertySchema::new("no_buttons", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_preview", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const INPUT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("label_text", PropertyValueKind::String, true, true),
            PropertySchema::enumerated("mode", true, true, &["text", "integer", "double", "item"]),
            PropertySchema::new("text_value", PropertyValueKind::String, true, true),
            PropertySchema::new("int_value", PropertyValueKind::Int, true, true),
            PropertySchema::new("double_value", PropertyValueKind::Float, true, true),
            // The integer step and the drop-down selection are assigned through the dialog's
            // own fields; `dropdown_open` reflects and drives the popup state.
            PropertySchema::new("int_step", PropertyValueKind::Int, true, true),
            PropertySchema::new("current_item", PropertyValueKind::UInt, true, true),
            PropertySchema::new("dropdown_open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROGRESS_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("label_text", PropertyValueKind::String, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            // Cancellation clears the value to the minimum; off keeps the position the
            // user cancelled at.
            PropertySchema::new("auto_reset", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_cancel_button", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_bar", PropertyValueKind::Bool, true, true),
            // The delay below which the dialog should not be shown; the host asks
            // `should_show_after(elapsed_ms)` rather than the control owning a clock.
            PropertySchema::new("minimum_duration", PropertyValueKind::UInt, true, true),
            // The bar's fill eases between values instead of jumping; off by default.
            PropertySchema::new("animated", PropertyValueKind::Bool, true, true),
            // Action colour instead of the success token; off by default.
            PropertySchema::new("accent_fill", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const POPUP_WINDOW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("has_content", PropertyValueKind::Bool, true, false),
            // A modal popup is kept up by an outside click; a non-modal one is dismissed.
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            // `0` means "no transient parent"; any other value is the owner's object id.
            PropertySchema::new("transient_parent", PropertyValueKind::UInt, true, true),
            // Rounded corners plus a blurred shadow; off keeps the square frame, which is
            // the frame the control has always painted.
            PropertySchema::new("floating_chrome", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("resizable", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("has_content", PropertyValueKind::Bool, true, false),
            // Read-only, like the wizard's navigation flags: the outcome is recorded by
            // finishing the dialog (`accept` / `reject`), and a writable `result` would let a
            // caller record an answer nobody gave. `closable` is a plain option bit, so it is
            // writable.
            PropertySchema::new("result", PropertyValueKind::Int, true, false),
            PropertySchema::new("accepted", PropertyValueKind::Bool, true, false),
            PropertySchema::new("rejected", PropertyValueKind::Bool, true, false),
            PropertySchema::new("closable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BOTTOM_SHEET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("peek_height", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const WIZARD_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            // The secondary line under the step title. Empty by default, so the resting
            // frame is unchanged.
            PropertySchema::new("subtitle", PropertyValueKind::String, true, true),
            // Read-only, like its three neighbours: the wizard has no random-access
            // step setter. Navigation moves through the `next` / `back` commands,
            // which emit `step_changed`, and `WizardDialog::set` refuses a direct
            // write. Declaring it writable made `set_current_step` a command whose
            // property route answers `ReadOnlyProperty`.
            PropertySchema::new("current_step", PropertyValueKind::UInt, true, false),
            PropertySchema::new("step_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("can_go_back", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_go_forward", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_ALERT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOLTIP_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // The tooltip's own shown-state. It is published as `shown`, not
            // `visible`, because `visible` is the base widget's visibility — the
            // `visible` is the base widget's visibility — the
            // tooltip's visibility is what `shown` drives, and collapsing the two
            // would leave one of them unreachable.
            PropertySchema::new("shown", PropertyValueKind::Bool, true, true),
            // Breaks the label at spaces and newlines and grows the bubble a line at a
            // time; off keeps the single-line eliding bubble.
            PropertySchema::new("wrap", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const POPOVER_PROPERTIES: &[PropertySchema] = &[
            // See `TOOLTIP_PROPERTIES`: the popup's own shown-state is `shown`,
            // leaving `visible` to mean the base widget's visibility.
            PropertySchema::new("shown", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            // Read-only: the text mirrors the installed content widget, so a write
            // would either be discarded by the next layout or require synthesising a
            // label the caller never asked for. `Popover::set` answers
            // `ReadOnlyProperty` and `set_content` is the way to change it.
            PropertySchema::new("text", PropertyValueKind::String, true, false),
            // `transient` / `semi_transient` / `permanent` — the dismissal rule, published
            // as an enumeration so a designer offers the three names rather than a boolean
            // that could not express the middle case.
            PropertySchema::enumerated(
                "behavior",
                true,
                true,
                &["transient", "semi_transient", "permanent"],
            ),
            // `auto` keeps the original below-or-above rule; the fixed directions pin the
            // card to one side of its anchor.
            PropertySchema::enumerated(
                "arrow_direction",
                true,
                true,
                &["auto", "up", "down", "left", "right"],
            ),
            // Empty means "use the built-in label", which is what keeps the default frame
            // unchanged.
            PropertySchema::new("placeholder_text", PropertyValueKind::String, true, true),
            PropertySchema::new("translucent", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MODAL_BOTTOM_SHEET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            // The panel's height in logical pixels; `0` restores the height derived from the
            // control, so the override is clearable over the property route too.
            PropertySchema::new("sheet_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("animated", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FIND_REPLACE_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("find_text", PropertyValueKind::String, true, true),
            PropertySchema::new("replace_text", PropertyValueKind::String, true, true),
            PropertySchema::new("match_case", PropertyValueKind::Bool, true, true),
            // The search grammar and the highlight switch; each has a dedicated setter on the
            // dialog.
            PropertySchema::new("whole_word", PropertyValueKind::Bool, true, true),
            PropertySchema::new("use_regex", PropertyValueKind::Bool, true, true),
            PropertySchema::new("highlight_all", PropertyValueKind::Bool, true, true),
            PropertySchema::new("wrap_around", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // ── Modal notices ──────────────────────────────────────────────────

        #[cfg(not(alloc_frugal))]
        pub(crate) const ALERT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("button_text", PropertyValueKind::String, true, true),
            // The notice card's shared surface: supplementary text, the optional checkbox, the
            // default button index and the icon name (`""` clears it). All are served by the
            // dialog's own accessors.
            PropertySchema::new("informative_text", PropertyValueKind::String, true, true),
            PropertySchema::new("detailed_text", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("default_button", PropertyValueKind::UInt, true, true),
            PropertySchema::new("check_box", PropertyValueKind::String, true, true),
            PropertySchema::new("check_box_checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CONFIRM_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("accept_text", PropertyValueKind::String, true, true),
            PropertySchema::new("reject_text", PropertyValueKind::String, true, true),
            // The same notice card as `alert_dialog`, served by this dialog's own accessors.
            PropertySchema::new("informative_text", PropertyValueKind::String, true, true),
            PropertySchema::new("detailed_text", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("default_button", PropertyValueKind::UInt, true, true),
            PropertySchema::new("check_box", PropertyValueKind::String, true, true),
            PropertySchema::new("check_box_checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ABOUT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("application_name", PropertyValueKind::String, true, true),
            PropertySchema::new("version", PropertyValueKind::String, true, true),
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            // The same notice card as `alert_dialog`, served by this dialog's own accessors.
            PropertySchema::new("informative_text", PropertyValueKind::String, true, true),
            PropertySchema::new("detailed_text", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("default_button", PropertyValueKind::UInt, true, true),
            PropertySchema::new("check_box", PropertyValueKind::String, true, true),
            PropertySchema::new("check_box_checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
