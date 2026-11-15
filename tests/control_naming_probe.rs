// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration-level construction probe for controls whose only tests were inline (W8-03).
//!
//! §11.1 of the 2026-11-15 scan found 45 canonical controls named nowhere under `tests/`: each
//! had inline `#[cfg(test)]` coverage, but nothing exercised it through the published factory —
//! the layer a host actually uses. This file is the cluster probe the scan recommended rather
//! than one file per name.
//!
//! Covered names (keep this list in step with the audit; a newly uncovered control belongs here):
//!
//! `about_dialog`, `alert_dialog`, `align`, `aspect_ratio`, `axis_picker`, `chart_crosshair`,
//! `chart_reference_line`, `chart_tooltip`, `command_palette_host`, `confirm_dialog`, `dock_area`,
//! `drag_ghost_layer`, `drop_down_popup_host`, `drop_zone`, `find_replace_bar`, `flex`,
//! `flexible`, `flow_layout`, `grid_status_bar`, `grid_view_header`, `group_avatar`, `header_menu`,
//! `intrinsic_width`, `key_sequence_editor`, `line_number_gutter`, `list_state_host`,
//! `mac_segmented_control`, `mac_source_list`, `mac_stepper`, `mac_toolbar`, `mac_window_chrome`,
//! `markdown_editor`, `markdown_toolbar`, `preset_chip_bar`, `shape_canvas`, `sized_box`, `spacer`,
//! `split_button`, `summary_row`, `tag_group`, `timeline_widget`, `tooltip_manager`, `tree_find_bar`,
//! `zoom_pan_controller`, `zoom_strip`.
#![cfg(full_widgets)]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

#[test]
fn the_controls_without_a_dedicated_integration_test_construct_through_the_factory() {
    const NAMES: &[&str] = &[
        "about_dialog",
        "alert_dialog",
        "align",
        "aspect_ratio",
        "axis_picker",
        "chart_crosshair",
        "chart_reference_line",
        "chart_tooltip",
        "command_palette_host",
        "confirm_dialog",
        "dock_area",
        "drag_ghost_layer",
        "drop_down_popup_host",
        "drop_zone",
        "find_replace_bar",
        "flex",
        "flexible",
        "flow_layout",
        "grid_status_bar",
        "grid_view_header",
        "group_avatar",
        "header_menu",
        "intrinsic_width",
        "key_sequence_editor",
        "line_number_gutter",
        "list_state_host",
        "mac_segmented_control",
        "mac_source_list",
        "mac_stepper",
        "mac_toolbar",
        "mac_window_chrome",
        "markdown_editor",
        "markdown_toolbar",
        "preset_chip_bar",
        "shape_canvas",
        "sized_box",
        "spacer",
        "split_button",
        "summary_row",
        "tag_group",
        "timeline_widget",
        "tooltip_manager",
        "tree_find_bar",
        "zoom_pan_controller",
        "zoom_strip",
    ];

    let factory = WidgetFactory::new_with_defaults();
    let geometry = Rect::new(0, 0, 240, 120);
    for name in NAMES {
        assert!(
            factory.capability(name).is_some(),
            "`{name}` must publish a capability through the default factory"
        );
        let widget = factory.create(name, geometry, "").unwrap_or_else(|| {
            panic!("`{name}` is registered but the factory refused to construct it")
        });
        assert_eq!(
            widget.geometry(),
            geometry,
            "`{name}` must take the geometry the factory was given"
        );
    }
}
