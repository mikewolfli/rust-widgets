// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! High-level image wrapper with decode, encode, and transform capabilities.

use crate::image::format::{DecodedImage, ImageData};
use crate::image::{decoder, encoder, transform};

/// High-level image wrapper with decode, encode, and transform capabilities.
#[derive(Debug, Clone)]
pub struct Image {
    /// The underlying decoded image data.
    pub inner: DecodedImage,
}

impl Image {
    /// Creates an empty image for widgets that have no source yet.
    pub fn new() -> Self {
        Self::from_raw(ImageData::Rgba8(Vec::new()), 0, 0)
    }

    /// Creates an image from validated raw RGBA8 pixels, refusing a size mismatch.
    ///
    /// This compatibility constructor preserves the historical widget-image API while storing
    /// pixels in the canonical `ImageData` representation.
    ///
    /// # Why `Result` rather than a panic
    ///
    /// It used to `assert_eq!` the length against `width * height * 4`. That is a programming
    /// error in the *caller's* arithmetic, not an invariant of this type, and a public constructor
    /// has no business aborting the host process over it — a decoder that produced a short buffer,
    /// or a caller who computed the size with saturating arithmetic, would take the whole
    /// application down. `Err` names the mismatch; `expect` at a call site that knows its own
    /// buffer is right is one line and reads better than an assertion buried in a library.
    pub fn try_from_rgba(data: Vec<u8>, width: u32, height: u32) -> Result<Self, String> {
        let expected = width as usize * height as usize * 4;
        if data.len() != expected {
            return Err(format!(
                "Image::try_from_rgba: data length {} does not match {width}x{height} RGBA \
                 (expected {expected})",
                data.len()
            ));
        }
        Ok(Self::from_raw(ImageData::Rgba8(data), width, height))
    }

    /// Decode an image from raw bytes. Auto-detects format.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let decoded = decoder::decode(data)?;
        Ok(Self { inner: decoded })
    }

    /// Decode RGBA8 from bytes without consulting the decode cache.
    ///
    /// This is the *private decode* entry point: a caller that wants its own copy (loading a
    /// series of frames, say) gets one. A caller that expects to share a decode **must** use
    /// [`crate::image::cache::pixels_rgba8_or_none`], which returns the cached `Arc<Vec<u8>>`
    /// directly — the controls' draw paths all go through it.
    ///
    /// There used to be a `from_bytes_rgba8_shared` beside this one. It went to the cache and
    /// then copied the pixels into an owning `Image`, so it saved the decode but performed the
    /// very copy its name promised to avoid, and nothing in the crate ever called it. An owner
    /// that needs both can write
    /// `Image::from_raw(ImageData::Rgba8(pixels.to_vec()), w, h)` against the cache entry, which
    /// at least says out loud that a copy is happening.
    pub fn from_bytes_rgba8(data: &[u8]) -> Result<Self, String> {
        let decoded = decoder::decode_to_rgba8(data)?;
        Ok(Self { inner: decoded })
    }

    /// Create from a DecodedImage.
    pub fn from_decoded(decoded: DecodedImage) -> Self {
        Self { inner: decoded }
    }

    /// Create from raw pixel data.
    pub fn from_raw(data: ImageData, width: u32, height: u32) -> Self {
        Self {
            inner: DecodedImage::new(crate::image::format::ImageFormat::Rgba8, data, width, height),
        }
    }

    /// Image width in pixels.
    pub fn width(&self) -> u32 {
        self.inner.width
    }

    /// Image height in pixels.
    pub fn height(&self) -> u32 {
        self.inner.height
    }

    /// Image format.
    pub fn format(&self) -> crate::image::format::ImageFormat {
        self.inner.format
    }

    /// Reference to pixel data.
    pub fn data(&self) -> &ImageData {
        &self.inner.data
    }

    /// Returns RGBA8 bytes when the underlying image is already RGBA8.
    pub fn rgba8_data(&self) -> Option<&[u8]> {
        match &self.inner.data {
            ImageData::Rgba8(data) => Some(data),
            _ => None,
        }
    }

    /// Returns whether the image has no pixels.
    pub fn is_empty(&self) -> bool {
        self.inner.data.as_bytes().is_empty()
    }

    /// Loads an image from a file path.
    pub fn from_file(path: &str) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|error| {
            format!(
                "image file '{path}' could not be read: {error} (check that the path exists \
                     and is readable)"
            )
        })?;
        Self::from_bytes(&data)
    }

    /// Returns RGBA8 pixel data, converting if needed.
    pub fn to_rgba8(&self) -> ImageData {
        self.inner.as_rgba8()
    }

    /// Encode the image to bytes in the specified format.
    pub fn encode(&self, format: crate::image::format::ImageFormat) -> Result<Vec<u8>, String> {
        encoder::encode(&self.inner, format)
    }

    /// Resize the image to new dimensions.
    pub fn resize(&self, width: u32, height: u32) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let resized = transform::resize(rgba, self.inner.width, self.inner.height, width, height)?;
        Ok(Self::from_raw(resized, width, height))
    }

    /// Crop a rectangular region from the image.
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let cropped =
            transform::crop(rgba, self.inner.width, self.inner.height, x, y, width, height)?;
        Ok(Self::from_raw(cropped, width, height))
    }

    /// Rotate the image by 90, 180, or 270 degrees.
    pub fn rotate(&self, degrees: u32) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let (rotated, w, h) =
            transform::rotate(rgba, self.inner.width, self.inner.height, degrees)?;
        Ok(Self::from_raw(rotated, w, h))
    }

    /// Flip the image horizontally (mirror).
    pub fn flip_horizontal(&self) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let flipped = transform::flip_horizontal(rgba, self.inner.width, self.inner.height)?;
        Ok(Self::from_raw(flipped, self.inner.width, self.inner.height))
    }

    /// Flip the image vertically.
    pub fn flip_vertical(&self) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let flipped = transform::flip_vertical(rgba, self.inner.width, self.inner.height)?;
        Ok(Self::from_raw(flipped, self.inner.width, self.inner.height))
    }
}

crate::impl_default_via_new!(Image);

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a small test image (2×2 RGBA8 with known pixels).
    fn test_image() -> Image {
        let data = ImageData::Rgba8(vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ]);
        Image::from_raw(data, 2, 2)
    }

    #[test]
    fn image_from_raw() {
        let img = test_image();
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
    }

    /// A buffer whose length disagrees with its declared size is an `Err`, not an abort.
    ///
    /// The constructor used to `assert_eq!`, so a caller whose decoder produced a short buffer
    /// took the host process down — from a library, over an arithmetic mistake the `Err` can
    /// describe. This pins the refusal and that the message names the expected length, because a
    /// bare "mismatch" would leave the caller to work it out.
    #[test]
    fn a_size_mismatch_is_refused_rather_than_panicking() {
        let error = Image::try_from_rgba(vec![0u8; 3], 1, 1)
            .expect_err("three bytes are not one RGBA pixel");
        assert!(error.contains("expected 4"), "the error must name the expected length: {error}");
        assert!(Image::try_from_rgba(vec![0u8; 4], 1, 1).is_ok());
    }

    #[test]
    fn compatibility_rgba_constructor_exposes_canonical_pixels() {
        let img = Image::try_from_rgba(vec![255, 0, 0, 255], 1, 1)
            .expect("the buffer matches its declared size");
        assert_eq!(img.format(), crate::image::format::ImageFormat::Rgba8);
        assert_eq!(img.rgba8_data(), Some(&[255, 0, 0, 255][..]));
        assert!(!img.is_empty());
    }

    #[test]
    fn image_width_height() {
        let img = Image::from_raw(ImageData::Rgba8(vec![0u8; 16 * 16 * 4]), 16, 16);
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
    }

    #[test]
    fn image_to_rgba8_roundtrip() {
        let img = test_image();
        let rgba = img.to_rgba8();
        assert_eq!(rgba.as_bytes().len(), 16); // 4 pixels * 4 bytes
    }

    #[test]
    fn image_resize_maintains_aspect() {
        let img = Image::from_raw(ImageData::Rgba8(vec![0u8; 100 * 100 * 4]), 100, 100);
        let resized = img.resize(50, 50).unwrap();
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 50);
    }

    #[test]
    fn image_crop_returns_correct_size() {
        let img = Image::from_raw(ImageData::Rgba8(vec![0u8; 100 * 100 * 4]), 100, 100);
        let cropped = img.crop(10, 10, 20, 30).unwrap();
        assert_eq!(cropped.width(), 20);
        assert_eq!(cropped.height(), 30);
    }

    #[test]
    fn image_flip_horizontal_works() {
        let img = test_image();
        let flipped = img.flip_horizontal().unwrap();
        assert_eq!(flipped.width(), 2);
        assert_eq!(flipped.height(), 2);
    }

    #[test]
    fn image_flip_vertical_works() {
        let img = test_image();
        let flipped = img.flip_vertical().unwrap();
        assert_eq!(flipped.width(), 2);
        assert_eq!(flipped.height(), 2);
    }

    #[test]
    fn image_crop_invalid_returns_err() {
        let img = Image::from_raw(ImageData::Rgba8(vec![0u8; 10 * 10 * 4]), 10, 10);
        assert!(img.crop(5, 5, 10, 10).is_err());
    }

    #[test]
    fn image_resize_zero_returns_err() {
        let img = Image::from_raw(ImageData::Rgba8(vec![0u8; 10 * 10 * 4]), 10, 10);
        assert!(img.resize(0, 0).is_err());
    }
}
