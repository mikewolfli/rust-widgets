// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Media widget types — rich content and media playback controls.
//!
//! # Gating
//!
//! The whole folder is declared `#[cfg(full_widgets)]` by `src/widget/mod.rs`, so
//! nothing inside needs a second profile gate. `full_widgets` is
//! `device_profile && !stripped`, which already excludes `mini` and `embedded`
//! (`not(alloc_frugal)` and `widgets_unstripped` respectively) — repeating those
//! predicates here produced gates that were true wherever the folder compiled at
//! all, which is the kind of redundancy that makes a later reader believe a
//! finer-grained gate is being enforced than really is.
//!
//! This doc comment used to claim "AnimatedImage is always available
//! (animated_image.rs has no cfg gate on mini)". That was false in the way that
//! matters: the module has no gate of its own, but the folder containing it is
//! excluded on the reduced profiles, so `AnimatedImage` is absent there too.

pub mod animated_image;
pub mod audio_visualizer;
pub mod camera_preview;
pub mod hero_animation;
pub mod lottie_widget;
pub mod rive_widget;
pub mod video_player;

pub use animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat};
pub use audio_visualizer::AudioVisualizer;
pub use camera_preview::CameraPreview;
pub use hero_animation::HeroAnimation;
pub use lottie_widget::LottieWidget;
pub use rive_widget::{RiveInput, RiveInputValue, RiveWidget};
pub use video_player::VideoPlayer;
