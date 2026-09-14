// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Media widget types — rich content and media playback controls.

#[cfg(not(alloc_frugal))]
pub mod animated_image;
#[cfg(not(alloc_frugal))]
pub mod audio_visualizer;
#[cfg(not(alloc_frugal))]
pub mod camera_preview;
#[cfg(not(alloc_frugal))]
pub mod hero_animation;
#[cfg(not(alloc_frugal))]
pub mod lottie_widget;
#[cfg(not(alloc_frugal))]
pub mod rive_widget;
#[cfg(not(alloc_frugal))]
pub mod video_player;

// AnimatedImage is always available (animated_image.rs has no cfg gate on mini)
pub use animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat};
#[cfg(not(alloc_frugal))]
pub use audio_visualizer::AudioVisualizer;
#[cfg(not(alloc_frugal))]
pub use camera_preview::CameraPreview;
#[cfg(not(alloc_frugal))]
pub use hero_animation::HeroAnimation;
#[cfg(not(alloc_frugal))]
pub use lottie_widget::LottieWidget;
#[cfg(not(alloc_frugal))]
pub use rive_widget::{RiveInput, RiveInputValue, RiveWidget};
#[cfg(not(alloc_frugal))]
pub use video_player::VideoPlayer;
