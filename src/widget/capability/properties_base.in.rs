// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_base {
    () => {
        pub(crate) const BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("pressed", PropertyValueKind::Bool, true, true),
            PropertySchema::new("default", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const LABEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "alignment",
                true,
                true,
                &["left", "centre", "right", "top", "bottom"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const CHECK_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // The tokens are the **reader's** vocabulary, not the writer's.
            //
            // This published `off` / `indeterminate` / `on` while `check_state_to_str` returns
            // `unchecked` / `partially_checked` / `checked`. The writer accepts both sets (it
            // parses the aliases deliberately), which is why the write-direction gate stayed
            // green on a genuinely mismatched pair. A consumer that builds a combo box from
            // this list — the designer's first requirement — would offer six values that
            // `get` never reports, and none of them would match the control's current state.
            // `readable_enum_properties_return_a_published_token` is the gate that keeps this
            // list honest in the direction that matters.
            PropertySchema::enumerated(
                "state",
                true,
                true,
                &["unchecked", "partially_checked", "checked"],
            ),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tristate_enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const RADIO_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("group_id", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOGGLE_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            // Read-only, and the tokens are the reader's three.
            //
            // `ToggleButton::state()` is **derived** — `disabled` from `enabled`, and
            // `checked`/`normal` from `checked` — so there is no independent state for a writer
            // to set. Declaring it writable (as an earlier revision of this table did) promises
            // a `set` that the control cannot honour, and the write-direction gate rejects the
            // contradiction immediately. Callers change the state through `enabled` and
            // `checked`, which are the two properties it is computed from.
            //
            // The tokens are `normal`/`checked`/`disabled` because that is what `get` returns;
            // this row previously published the *checkbox's* `off`/`indeterminate`/`on`, a
            // vocabulary this control has never used.
            PropertySchema::enumerated("state", true, false, &["normal", "checked", "disabled"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const LINE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
