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
    /// Decode an image from raw bytes. Auto-detects format.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let decoded = decoder::decode(data)?;
        Ok(Self { inner: decoded })
    }

    /// Decode from bytes and convert to RGBA8.
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
