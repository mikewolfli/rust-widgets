// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_view_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_list_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ListView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_tree_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TreeView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_table(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Table, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_data_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DataView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_property_grid(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::PropertyGrid, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_column_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ColumnView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_undo_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::UndoView, parent, "", x, y, width, height)
        }
        // The four kinds promoted from shared kinds in the 2.4.0 audit. Without
        // these arms their `WidgetKind` variants existed but no backend method
        // served them, so the route matrix graded them `Placeholder` and
        // `tools/check_control_route_matrix.sh` failed with status 3.
        #[cfg(not(alloc_frugal))]
        fn create_breadcrumb(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Breadcrumb, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_drop_zone(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DropZone, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_signature_pad(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SignaturePad, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_tree_table(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TreeTable, parent, "", x, y, width, height)
        }
    };
}
