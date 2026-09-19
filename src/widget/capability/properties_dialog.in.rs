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
            PropertySchema::new("modal", PropertyValueKind::Bool, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FILE_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("directory", PropertyValueKind::String, false, false),
            PropertySchema::new("selected_file", PropertyValueKind::String, false, false),
            PropertySchema::enumerated(
                "mode",
                false,
                false,
                &["text", "integer", "double", "item"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROGRESS_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("label_text", PropertyValueKind::String, true, true),
            PropertySchema::new("value", PropertyValueKind::Int, false, false),
            PropertySchema::new("minimum", PropertyValueKind::Int, false, false),
            PropertySchema::new("maximum", PropertyValueKind::Int, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const POPUP_WINDOW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("has_content", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("has_content", PropertyValueKind::Bool, true, false),
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
            // tooltip's visibility is what `shown` drives, and collapsing the two
            // would leave one of them unreachable.
            PropertySchema::new("shown", PropertyValueKind::Bool, true, true),
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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MODAL_BOTTOM_SHEET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FIND_REPLACE_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("find_text", PropertyValueKind::String, true, true),
            PropertySchema::new("replace_text", PropertyValueKind::String, true, true),
            PropertySchema::new("match_case", PropertyValueKind::Bool, true, true),
            PropertySchema::new("wrap_around", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
