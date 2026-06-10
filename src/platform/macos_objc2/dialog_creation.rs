use super::types::{MacOSObjc2Platform, MacObjc2HandleKind};
use crate::core::ObjectId;
use crate::platform::Platform;

impl Platform for MacOSObjc2Platform {
    fn create_message_box(
        &self,
        parent: ObjectId,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, text);
        self.insert_widget(MacObjc2HandleKind::MessageBox, "MessageBox", x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(MacObjc2HandleKind::FileDialog, "FileDialog", x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(MacObjc2HandleKind::ColorDialog, "ColorDialog", x, y, width, height)
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(MacObjc2HandleKind::FontDialog, "FontDialog", x, y, width, height)
    }
}
