// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Modern widget-set create methods (BLUE13 R2.1-R2.14 + mobile/Cupertino/media
// widget families).
//
// Expanded via `impl_modern_widgets!()` inside the
// `impl ControlBackend for CustomPaintControlBackend` block in
// `create_widgets.rs`. Method bodies are written literally (same pattern as the
// other `create_widgets_*.in.rs` files) so the control-route matrix tooling can
// parse the real implementations; each method builds the widget through
// `WidgetFactory` and mounts it, so the returned id addresses a live control.
//
// Methods for `WidgetKind` variants that exist in every profile (Arc, Switch,
// Frame, ...) are emitted ungated; variants gated by
// `#[cfg(not(alloc_frugal))]` in `src/widget/kind.rs` carry the same gate
// here so the `mini` profile still compiles.

macro_rules! impl_modern_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_adaptive_scaffold(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AdaptiveScaffold, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_animated_image(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AnimatedImage, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_app_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AppBar, parent, "", x, y, width, height)
        }

        fn create_arc(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Arc, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_audio_visualizer(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AudioVisualizer, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_auto_complete_edit(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AutoCompleteEdit, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_avatar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Avatar, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_badge(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Badge, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_bar_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::BarChart, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_barcode_scanner(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::BarcodeScanner, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_bezier_curve_editor(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::BezierCurveEditor,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_bottom_navigation_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::BottomNavigationBar,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_bottom_sheet(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::BottomSheet, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_camera_preview(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CameraPreview, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_carousel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Carousel, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_chip(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Chip, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_color_history(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ColorHistory, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_color_well(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ColorWell, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_alert_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::CupertinoAlertDialog,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_date_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::CupertinoDatePicker,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_navigation_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::CupertinoNavigationBar,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_segmented_control(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::CupertinoSegmentedControl,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_slider(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CupertinoSlider, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_cupertino_switch(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CupertinoSwitch, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_date_range_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DateRangePicker, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_divider(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Divider, parent, "", x, y, width, height)
        }

        fn create_dropdown(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Dropdown, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_dropdown_menu(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DropdownMenu, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_editable_combo_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::EditableComboBox, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_empty_state(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::EmptyState, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_fab(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FAB, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_find_replace_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::FindReplaceDialog,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_floating_label(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FloatingLabel, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_font_preview(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FontPreview, parent, "", x, y, width, height)
        }

        fn create_frame(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Frame, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_grid_table(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::GridTable, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_hero_animation(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::HeroAnimation, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_icon(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Icon, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_image_gallery(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ImageGallery, parent, "", x, y, width, height)
        }

        fn create_image_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ImageView, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_ime_preedit(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ImePreedit, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_inplace_editor(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::InplaceEditor, parent, "", x, y, width, height)
        }

        fn create_keyboard(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Keyboard, parent, "", x, y, width, height)
        }

        fn create_line(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Line, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_line_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::LineChart, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_lottie_widget(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::LottieWidget, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_masked_edit(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MaskedEdit, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_masonry_layout(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MasonryLayout, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_material_navigation_rail(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::MaterialNavigationRail,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_material_snackbar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MaterialSnackbar, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_menu_button(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MenuButton, parent, "", x, y, width, height)
        }

        fn create_meter(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Meter, parent, "", x, y, width, height)
        }

        fn create_mini_canvas(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MiniCanvas, parent, "", x, y, width, height)
        }

        fn create_mini_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MiniChart, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_mobile_date_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MobileDatePicker, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_modal_bottom_sheet(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ModalBottomSheet, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_multi_select_combo_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::MultiSelectComboBox,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }

        #[cfg(not(alloc_frugal))]
        fn create_navigation_drawer(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::NavigationDrawer, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_navigation_stack(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::NavigationStack, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_pie_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::PieChart, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_popover(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Popover, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_progress_circle(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ProgressCircle, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_properties_panel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::PropertiesPanel, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_qr_code(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::QRCode, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_range_slider(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RangeSlider, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_rating(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Rating, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_refresh_control(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RefreshControl, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_rive_widget(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RiveWidget, parent, "", x, y, width, height)
        }

        fn create_roller(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Roller, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_safe_area(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SafeArea, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_search_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SearchBar, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_search_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SearchBox, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_segmented_button(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SegmentedButton, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_shortcut_editor(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ShortcutEditor, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_skeleton_loader(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SkeletonLoader, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_number_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::NumberPicker, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_otp_input(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::OtpInput, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_pagination(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Pagination, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_banner(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Banner, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_color_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ColorPicker, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_toast(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Toast, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_splash_screen(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SplashScreen, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_sparkline(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Sparkline, parent, "", x, y, width, height)
        }

        fn create_spinner(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Spinner, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_stepper(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Stepper, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_swipe_to_dismiss(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SwipeToDismiss, parent, "", x, y, width, height)
        }

        fn create_switch(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Switch, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_tab_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TabView, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_tag_input(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TagInput, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_text_area(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TextArea, parent, "", x, y, width, height)
        }

        #[cfg(widgets_unstripped)]
        fn create_tooltip(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Tooltip, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_video_player(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::VideoPlayer, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_wizard_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::WizardDialog, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_freeform_shape(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FreeformShape, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_tab_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TabBar, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_pie_menu(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::PieMenu, parent, "", x, y, width, height)
        }

        #[cfg(not(alloc_frugal))]
        fn create_ribbon_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RibbonBar, parent, "", x, y, width, height)
        }
    };
}
