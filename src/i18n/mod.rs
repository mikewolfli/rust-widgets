//! i18n module - internationalization support with hot reload
//!
//! # Example
//!
//! ```rust
//! use rust_widgets::i18n::I18nManager;
//!
//! // Create an i18n manager
//! let mut manager = I18nManager::new();
//!
//! // Translate a key (returns the key itself if no translation found)
//! let greeting = manager.translate("hello");
//! assert_eq!(greeting, "hello");
//!
//! // Load translations from a JSON file
//! // manager.load_translations("path/to/en.json").unwrap();
//! ```
mod global;
mod macros;
mod manager;
mod options;
#[cfg(test)]
mod tests;
mod types;
mod watcher;
pub use global::{
    check_and_reload_all, get_manager, init, init_with_options, translate, translate_with_context,
};
pub use manager::I18nManager;
pub use options::{InitOptions, InitReport};
pub use types::{ReloadEvent, Translation, TranslationFile};
pub use watcher::{init_with_hot_reload, process_reload_events, I18nFileWatcher};
