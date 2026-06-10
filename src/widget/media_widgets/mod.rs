//! Media widget types — rich content and media playback controls.

#[cfg(not(feature = "mini"))]
pub mod animated_image;
#[cfg(not(feature = "mini"))]
pub mod audio_visualizer;
#[cfg(not(feature = "mini"))]
pub mod camera_preview;
#[cfg(not(feature = "mini"))]
pub mod hero_animation;
#[cfg(not(feature = "mini"))]
pub mod lottie_widget;
#[cfg(not(feature = "mini"))]
pub mod rive_widget;
#[cfg(not(feature = "mini"))]
pub mod video_player;

// AnimatedImage is always available (animated_image.rs has no cfg gate on mini)
pub use animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat};
#[cfg(not(feature = "mini"))]
pub use audio_visualizer::AudioVisualizer;
#[cfg(not(feature = "mini"))]
pub use camera_preview::CameraPreview;
#[cfg(not(feature = "mini"))]
pub use hero_animation::HeroAnimation;
#[cfg(not(feature = "mini"))]
pub use lottie_widget::LottieWidget;
#[cfg(not(feature = "mini"))]
pub use rive_widget::{RiveInput, RiveInputValue, RiveWidget};
#[cfg(not(feature = "mini"))]
pub use video_player::VideoPlayer;
