//! Playback state machine for video/audio.

/// Playback state for media engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// No media loaded or stopped.
    Stopped,
    /// Currently playing.
    Playing,
    /// Paused.
    Paused,
    /// Buffering/waiting for data.
    Buffering,
    /// Playback finished.
    Ended,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

impl PlaybackState {
    /// Returns true if the player is actively playing.
    pub fn is_active(&self) -> bool {
        matches!(self, PlaybackState::Playing | PlaybackState::Buffering)
    }

    /// Returns true if playback can be resumed.
    pub fn can_resume(&self) -> bool {
        matches!(self, PlaybackState::Paused | PlaybackState::Stopped)
    }

    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PlaybackState::Stopped => "Stopped",
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Buffering => "Buffering",
            PlaybackState::Ended => "Ended",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_default() {
        assert_eq!(PlaybackState::default(), PlaybackState::Stopped);
    }

    #[test]
    fn test_playback_state_is_active() {
        assert!(PlaybackState::Playing.is_active());
        assert!(PlaybackState::Buffering.is_active());
        assert!(!PlaybackState::Paused.is_active());
        assert!(!PlaybackState::Stopped.is_active());
    }

    #[test]
    fn test_playback_state_labels() {
        assert_eq!(PlaybackState::Playing.label(), "Playing");
        assert_eq!(PlaybackState::Paused.label(), "Paused");
        assert_eq!(PlaybackState::Ended.label(), "Ended");
    }
}
