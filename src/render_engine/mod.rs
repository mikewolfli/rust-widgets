/// Embedded runtime state, task queue, and shared engine internals.
pub mod embedded;
/// Embedded render engine with independent lifecycle and resource registry.
pub mod embedded_engine;
/// Render engine trait — unified contract for native and embedded engines.
pub mod engine_trait;
/// Native desktop render engine backed by platform adapters.
pub mod native;
// Re-exports
pub use embedded::*;
pub use embedded_engine::*;
pub use engine_trait::*;
pub use native::*;
