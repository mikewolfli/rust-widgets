//! Dialog widgets.

pub mod color_dialog;
pub mod file_dialog;
pub mod font_dialog;
pub mod input_dialog;
pub mod message_box;
pub mod popup_window;
pub mod progress_dialog;

// Re-export dialog types
pub use color_dialog::ColorDialog;
pub use file_dialog::FileDialog;
pub use font_dialog::FontDialog;
pub use input_dialog::InputDialog;
pub use message_box::MessageBox;
pub use popup_window::PopupWindow;
pub use progress_dialog::ProgressDialog;
