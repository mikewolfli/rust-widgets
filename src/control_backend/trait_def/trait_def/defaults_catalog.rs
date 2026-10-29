// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Default bodies for the modern widget catalogue (`create_adaptive_scaffold`
//! through `create_wrap_layout`).
//!
//! Split from `defaults` only to keep this file under the 2000-line limit: the
//! two together are the original defaulted body. Names in the bodies resolve at
//! the invocation site in `core`.

macro_rules! default_modern_widgets {
    () => {
        /// Derive a full accessibility state for a widget, or `None` when the id addresses nothing.
        ///
        /// # Why this is the bridge and not a method on each backend
        ///
        /// [`crate::platform::accessibility::A11yState`] has eleven fields, and until this existed
        /// **nothing in production code built one from a live control** — so `checked` and `mixed` were
        /// structurally unreachable no matter what a check box published. A backends' `register_widget`
        /// had to be handed a state by the host, which meant the host had to re-derive the mapping.
        ///
        /// The default answers the same question from the widget trait's own accessors, which every
        /// backend already exposes through its name/value accessors, so a backend that does nothing gets
        /// the derivation and one that has richer information can override it.
        fn widget_a11y_state(
            &self,
            widget_id: ObjectId,
        ) -> Option<crate::platform::accessibility::A11yState> {
            let _ = widget_id;
            None
        }
        /// Set clipboard text.
        fn set_clipboard_text(&self, _text: &str) -> bool {
            false
        }
        /// Get clipboard text.
        fn get_clipboard_text(&self) -> String {
            String::new()
        }
        /// Begin drag operation.
        fn begin_drag(&self, _source: ObjectId, _mime_type: &str, _payload: &[u8]) -> bool {
            false
        }
        /// Poll next drop event.
        fn poll_drop_event(&self) -> Option<crate::platform::DropEvent> {
            None
        }

        // ── Modern widget set (BLUE13 R2.1–R2.14 + mobile/Cupertino/media families) ──
        //
        // These defaults return `0` as the explicit unsupported marker; backends that
        // support a widget kind override the corresponding method.

        /// Create adaptive scaffold control.
        fn create_adaptive_scaffold(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create animated image control.
        fn create_animated_image(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create app bar control.
        fn create_app_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create arc control.
        fn create_arc(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create audio visualizer control.
        fn create_audio_visualizer(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create auto-complete edit control.
        fn create_auto_complete_edit(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create avatar control.
        fn create_avatar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create badge control.
        fn create_badge(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create bar chart control.
        fn create_bar_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create barcode scanner control.
        fn create_barcode_scanner(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create bezier curve editor control.
        fn create_bezier_curve_editor(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create bottom navigation bar control.
        fn create_bottom_navigation_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create bottom sheet control.
        fn create_bottom_sheet(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create camera preview control.
        fn create_camera_preview(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create carousel control.
        fn create_carousel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create chip control.
        fn create_chip(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create color history control.
        fn create_color_history(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create color well control.
        fn create_color_well(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino alert dialog control.
        fn create_cupertino_alert_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino date picker control.
        fn create_cupertino_date_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino navigation bar control.
        fn create_cupertino_navigation_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino segmented control.
        fn create_cupertino_segmented_control(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino slider control.
        fn create_cupertino_slider(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create cupertino switch control.
        fn create_cupertino_switch(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create date range picker control.
        fn create_date_range_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create divider control.
        fn create_divider(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create dropdown control.
        fn create_dropdown(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create dropdown menu control.
        fn create_dropdown_menu(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create editable combo box control.
        fn create_editable_combo_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create empty state control.
        fn create_empty_state(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create floating action button (FAB) control.
        fn create_fab(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create find/replace dialog control.
        fn create_find_replace_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create floating label control.
        fn create_floating_label(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create font preview control.
        fn create_font_preview(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create frame control.
        fn create_frame(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create grid table control.
        fn create_grid_table(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create hero animation control.
        fn create_hero_animation(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create icon control.
        fn create_icon(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create image gallery control.
        fn create_image_gallery(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create image view control.
        fn create_image_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create IME preedit control.
        fn create_ime_preedit(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create in-place editor control.
        fn create_inplace_editor(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create keyboard control.
        fn create_keyboard(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create line control.
        fn create_line(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create line chart control.
        fn create_line_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create lottie widget control.
        fn create_lottie_widget(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create masked edit control.
        fn create_masked_edit(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create masonry layout control.
        fn create_masonry_layout(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create material navigation rail control.
        fn create_material_navigation_rail(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create material snackbar control.
        fn create_material_snackbar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create menu button control.
        fn create_menu_button(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create meter control.
        fn create_meter(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create mini canvas control.
        fn create_mini_canvas(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create mini chart control.
        fn create_mini_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create mobile date picker control.
        fn create_mobile_date_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create modal bottom sheet control.
        fn create_modal_bottom_sheet(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create multi-select combo box control.
        fn create_multi_select_combo_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create navigation drawer control.
        fn create_navigation_drawer(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create navigation stack control.
        fn create_navigation_stack(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create pie chart control.
        fn create_pie_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create popover control.
        fn create_popover(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create progress circle control.
        fn create_progress_circle(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create properties panel control.
        fn create_properties_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create QR code control.
        fn create_qr_code(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create QRCode control (alias for `create_qr_code` used by the route-matrix
        /// generator, which derives `create_qrcode` from `WidgetKind::QRCode`).
        fn create_qrcode(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create range slider control.
        fn create_range_slider(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create rating control.
        fn create_rating(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create refresh control.
        fn create_refresh_control(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create rive widget control.
        fn create_rive_widget(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create roller control.
        fn create_roller(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create safe area control.
        fn create_safe_area(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create search bar control.
        fn create_search_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create search box control.
        fn create_search_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create segmented button control.
        fn create_segmented_button(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create segmented control (single-select segmented bar).
        ///
        /// Separate from [`Self::create_segmented_button`]: the two controls share a shape but not a
        /// behaviour (multi-select with icons vs single-select with a sliding indicator), and they have
        /// separate `WidgetKind`s. Omitting this entry left the control reachable from the registry but
        /// not from the backend trait — a gap the earlier kind collision hid, because the control
        /// answered `WidgetKind::ToggleButton` and inherited the toggle's entry.
        fn create_segmented_control(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create shortcut editor control.
        fn create_shortcut_editor(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create skeleton loader control.
        fn create_skeleton_loader(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create number picker control.
        fn create_number_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create OTP input control.
        fn create_otp_input(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create pagination control.
        fn create_pagination(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create banner control.
        fn create_banner(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create colour picker control.
        fn create_color_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create toast control.
        fn create_toast(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create splash screen control.
        fn create_splash_screen(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create candlestick (K-line) chart control.
        fn create_candlestick_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }

        /// Create volume histogram control.
        fn create_volume_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }

        /// Create market depth chart control.
        fn create_depth_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }

        /// Create order book ladder control.
        fn create_order_book(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }

        /// Create quote board control.
        fn create_quote_board(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }

        /// Create technical indicator chart control.
        fn create_indicator_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create sparkline control.
        fn create_sparkline(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create spinner control.
        fn create_spinner(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create stepper control.
        fn create_stepper(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create swipe-to-dismiss control.
        fn create_swipe_to_dismiss(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create switch control.
        fn create_switch(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tab view control.
        fn create_tab_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tag input control.
        fn create_tag_input(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create breadcrumb trail control.
        ///
        /// Promoted from a shared `Panel` kind to its own kind in 2.4.0; the typed
        /// method is what keeps `WidgetKind::Breadcrumb` reachable by name from the
        /// bindings and the route matrix.
        fn create_breadcrumb(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create drag-and-drop upload target control.
        fn create_drop_zone(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create freehand signature capture control.
        fn create_signature_pad(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tree-table control (a tree whose rows carry editable columns).
        fn create_tree_table(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create text area control.
        fn create_text_area(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tooltip control.
        fn create_tooltip(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create video player control.
        fn create_video_player(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create wizard dialog control.
        fn create_wizard_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create about dialog control.
        fn create_about_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create accordion control.
        fn create_accordion(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create alert dialog control.
        fn create_alert_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create align wrapper control.
        fn create_align(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create aspect-ratio box control.
        fn create_aspect_ratio(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create centre wrapper control.
        fn create_center(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create clock face control.
        fn create_clock(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create confirm dialog control.
        fn create_confirm_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create expanded child control.
        fn create_expanded(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create flex distribution control.
        fn create_flex(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create flexible child control.
        fn create_flexible(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create flow layout control.
        fn create_flow_layout(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create gauge dial control.
        fn create_gauge(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create intrinsic-width wrapper control.
        fn create_intrinsic_width(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create padding wrapper control.
        fn create_padding(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create fixed-size box control.
        fn create_sized_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create spacer control.
        fn create_spacer(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tag control.
        fn create_tag(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create wrap layout control.
        fn create_wrap_layout(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
    };
}
pub(crate) use default_modern_widgets;
