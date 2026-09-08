pub(crate) mod create_widgets;
pub mod custom_paint;

pub use custom_paint::CustomPaintControlBackend;

#[cfg(all(test, not(feature = "embedded")))]
mod tests;
