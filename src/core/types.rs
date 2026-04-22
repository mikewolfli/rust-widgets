use std::fmt::Debug;

/// Stable numeric identifier used for widgets and objects.
pub type ObjectId = u64;

/// Runtime profile controlling feature and backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    /// Full desktop-oriented profile with optional advanced modules.
    Full,
    /// Reduced profile intended for constrained environments.
    Embedded,
}

/// Platform family classification for backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    /// Traditional desktop runtime targets.
    Desktop,
    /// Embedded and constrained runtime targets.
    Embedded,
    /// Mobile runtime targets.
    Mobile,
}

/// Common trait implemented by id-addressable core objects.
pub trait CoreObject: Debug + Send + Sync {
    /// Get stable object id.
    fn id(&self) -> ObjectId;
    /// Set stable object id (used by object system adapters).
    fn set_id(&mut self, id: ObjectId);
}
