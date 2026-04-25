//! Web capabilities module — provides web view, web engine, navigation, history,
//! JavaScript bridge, privacy controls, and plugin management.
mod history;
mod js_engine;
mod navigation;
mod plugins;
mod privacy;
mod web_core;
mod web_engine;
mod web_view;
pub use history::*;
pub use js_engine::*;
pub use navigation::*;
pub use plugins::*;
pub use privacy::*;
pub use web_engine::*;
pub use web_view::*;
