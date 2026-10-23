// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_container_widgets {
    () => {
        fn create_scroll_area(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ScrollArea, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_dock_panel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DockPanel, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_tab_widget(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TabWidget, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_splitter(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Splitter, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_stack_widget(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::StackedWidget, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_mdi_area(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MdiArea, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_toolbox(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            // By name, so the two spellings of this control (`tool_box`, and the
            // `toolbox` alias the factory also accepts) both land on the same
            // constructor rather than one of them depending on kind resolution.
            // A profile without the factory has no constructors either way and
            // reports `0`, so it keeps the kind-based call.
            #[cfg(full_widgets)]
            {
                self.mount_named_widget("toolbox", parent, "", x, y, width, height)
            }
            #[cfg(not(full_widgets))]
            {
                self.mount_widget_of_kind(WidgetKind::Toolbox, parent, "", x, y, width, height)
            }
        }
        #[cfg(widgets_unstripped)]
        fn create_collapsible_pane(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::CollapsiblePane,
                parent,
                title,
                x,
                y,
                width,
                height,
            )
        }
        #[cfg(widgets_unstripped)]
        fn create_dock_widget(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DockWidget, parent, title, x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_spacer(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Spacer, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_center(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Center, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_align(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Align, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_sized_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SizedBox, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_padding(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Padding, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_expanded(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Expanded, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_flexible(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Flexible, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_intrinsic_width(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::IntrinsicWidth, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_aspect_ratio(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AspectRatio, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_flex(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Flex, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_flow_layout(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FlowLayout, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_wrap_layout(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::WrapLayout, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_accordion(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Accordion, parent, "", x, y, width, height)
        }
    };
}
