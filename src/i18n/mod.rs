//! i18n module - internationalization support with hot reload

mod global;
mod macros;
mod manager;
mod options;
mod tests;
mod types;
mod watcher;

pub use global::{check_and_reload_all, get_manager, init, init_with_options, translate};
pub use macros::tr;
pub use manager::I18nManager;
pub use options::{InitOptions, InitReport};
pub use types::{ReloadEvent, Translation, TranslationFile};
pub use watcher::{init_with_hot_reload, process_reload_events, I18nFileWatcher};
