// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The sample payloads that are **pixels**: a gallery of photographs, one photograph, and a
//! two-frame animation.
//!
//! These three arms are the only ones in `apply` that build a pixel buffer, and they are the only
//! ones with a `feature = "image"` split (with the decoder the picture is decoded from real bytes;
//! without it the arm has to say something honest without an asset). Keeping them together means
//! the "what does an empty control of this family look like" decision is written once, beside the
//! assets it names, instead of interleaved with the 200 arms that set a scalar.

use crate::compat::Vec;
use crate::widget::display_widgets::image_view::{ImageAlignment, ImageAspect, ImageView};
use crate::widget::media_widgets::animated_image::{
    AnimatedFrame, AnimatedImage, AnimatedImageFit,
};
use crate::widget::view_widgets::image_gallery::ImageGallery;

/// Fills an empty gallery from `sample_photos/` (or with captioned placeholders, without the
/// `image` feature).
///
/// A gallery whose own snapshot says "No images in gallery" teaches a reviewer nothing about the
/// grid, the captions or the selection ring. The samples are decoded from real bytes, so the
/// snapshot exercises decode-and-paint rather than drawing the placeholder well four times.
pub(super) fn fill_gallery(gallery: &mut ImageGallery) {
    if !gallery.images().is_empty() {
        return;
    }
    #[cfg(feature = "image")]
    {
        use crate::widget::view_widgets::image_gallery::GalleryImage;
        const SAMPLES: [(&str, &str, &[u8]); 4] = [
            ("Coast", "coast.jpg", include_bytes!("../sample_photos/coast.jpg")),
            ("Forest", "forest.jpg", include_bytes!("../sample_photos/forest.jpg")),
            ("City", "city.jpg", include_bytes!("../sample_photos/city.jpg")),
            ("Desert", "desert.jpg", include_bytes!("../sample_photos/desert.jpg")),
        ];
        let mut images = Vec::with_capacity(SAMPLES.len());
        for (label, path, bytes) in SAMPLES {
            let mut image = GalleryImage::with_label(path, Some(label), 0, 0);
            if image.load_from_bytes(bytes).is_ok() {
                images.push(image);
            }
        }
        if !images.is_empty() {
            gallery.set_images(images);
        }
    }
    #[cfg(not(feature = "image"))]
    {
        for index in 1..=4 {
            gallery.add_image(&format!("sample-{index}.png"), Some(&format!("Image {index}")));
        }
    }
}

/// Loads the photograph into an empty `ImageView`.
///
/// `Fit` plus centre alignment states how the picture should be seen: the default
/// `Stretch`/unscaled pair paints only the source's own pixel count. Returns whether a picture was
/// actually loaded, so a `false` tells the caller the arm did nothing.
pub(super) fn fill_image_view(view: &mut ImageView) -> bool {
    #[cfg(feature = "image")]
    {
        let loaded =
            view.set_image_from_bytes(include_bytes!("../sample_photos/coast.jpg")).is_ok();
        view.set_aspect(ImageAspect::Fit);
        view.set_image_alignment(ImageAlignment::Center);
        loaded
    }
    // Without the `image` feature there is no decoder to feed, so the arm keeps a generated
    // checkerboard: no asset needed and visible at any size.
    #[cfg(not(feature = "image"))]
    {
        const SIDE: u32 = 32;
        let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
        for y in 0..SIDE {
            for x in 0..SIDE {
                let light = (x / 8 + y / 8) % 2 == 0;
                let (r, g, b) = if light { (33, 150, 243) } else { (236, 240, 245) };
                pixels.extend_from_slice(&[r, g, b, 255]);
            }
        }
        view.set_image(crate::image::Image::from_raw(
            crate::image::ImageData::Rgba8(pixels),
            SIDE,
            SIDE,
        ));
        view.set_aspect(ImageAspect::Fit);
        view.set_image_alignment(ImageAlignment::Center);
        true
    }
}

/// Loads two frames into an empty `AnimatedImage`.
///
/// The default `Original` fit draws at the frame's own pixels, so 24×24 frames were a speck in the
/// middle of the box. A 120×80 two-colour gradient plus `Contain` makes the picture read as a
/// picture, and the two phases differ so the frame pair is visibly an animation rather than one
/// still.
pub(super) fn fill_animated_image(image: &mut AnimatedImage) -> bool {
    const WIDTH: u32 = 120;
    const HEIGHT: u32 = 80;
    let mut frames = Vec::new();
    for phase in [0u32, 1] {
        let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let t = (x + phase * WIDTH / 2) % WIDTH;
                let r = (33 + (t * 180 / WIDTH)) as u8;
                let g = (150 - (y * 90 / HEIGHT)) as u8;
                let b = (243 - (t * 60 / WIDTH)) as u8;
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }
        frames.push(AnimatedFrame { data, width: WIDTH, height: HEIGHT, delay_ms: 400 });
    }
    image.set_fit(AnimatedImageFit::Contain);
    image.load_frames(frames).is_ok()
}
