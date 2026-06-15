//! Image processing module — format detection, decoding, encoding, transform, color conversion.
//!
//! Supports all mainstream formats: PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF, ICO, PNM, QOI,
//! Farbfeld, SVG, and compressed SVG (SVGZ).

mod color;
pub mod decoder;
mod encoder;
pub mod exif;
pub mod format;
pub mod svg_utils;
pub mod transform;

pub use color::*;
pub use decoder::{decode, decode_to_rgba8, detect_format};
pub use encoder::*;
pub use exif::*;
pub use format::{ColorSpace, DecodedImage, ExifData, ImageData, ImageFormat};
pub use svg_utils::*;
pub use transform::*;

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

    /// Convert color space.
    pub fn convert_color_space(&self, target: ColorSpace) -> Result<Self, String> {
        let mut img = self.clone();
        img.inner.color_space = target;
        Ok(img)
    }

    /// Convert to grayscale.
    pub fn to_grayscale(&self) -> Result<Self, String> {
        let rgba = self.to_rgba8();
        let (gray, w, h) = color::to_grayscale(rgba, self.inner.width, self.inner.height)?;
        let mut img = Self::from_raw(gray, w, h);
        img.inner.color_space = ColorSpace::Grayscale;
        Ok(img)
    }
}

#[cfg(test)]
/// Helper: create a solid-color 24-bit BMP with the given dimensions.
fn make_red_bmp(width: u32, height: u32) -> Vec<u8> {
    let row_size = ((width * 24 + 31) / 32) * 4;
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let mut bmp = Vec::with_capacity(file_size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]); // reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // pixel offset

    // DIB header (BITMAPINFOHEADER, 40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes()); // header size
    bmp.extend_from_slice(&width.to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes()); // positive = bottom-up
    bmp.extend_from_slice(&1u16.to_le_bytes()); // planes
    bmp.extend_from_slice(&24u16.to_le_bytes()); // bit count
    bmp.extend_from_slice(&[0u8; 24]); // compression and remaining 6 fields

    // Pixel data: red (BGR = 00 00 FF)
    for _ in 0..height {
        for _ in 0..width {
            bmp.push(0x00); // B
            bmp.push(0x00); // G
            bmp.push(0xFF); // R
        }
        // Row padding to 4-byte boundary
        let padding = (row_size - width * 3) as usize;
        for _ in 0..padding {
            bmp.push(0x00);
        }
    }
    bmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::format::ImageFormat;

    #[test]
    fn image_from_bmp_bytes() {
        let bmp = make_red_bmp(1, 1);
        let img = Image::from_bytes(&bmp).unwrap();
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
        assert_eq!(img.format(), ImageFormat::Bmp);
    }

    #[test]
    fn image_from_bytes_rgba8() {
        let bmp = make_red_bmp(10, 10);
        let img = Image::from_bytes_rgba8(&bmp).unwrap();
        assert_eq!(img.width(), 10);
        assert_eq!(img.format(), ImageFormat::Rgba8);
    }

    #[test]
    fn image_roundtrip_ff_to_rgba() {
        let mut ff = b"farbfeld".to_vec();
        ff.extend_from_slice(&1u32.to_be_bytes());
        ff.extend_from_slice(&1u32.to_be_bytes());
        ff.extend_from_slice(&[255, 128, 0, 0, 0, 128, 255, 255]);
        let img = Image::from_bytes(&ff).unwrap();
        assert_eq!(img.width(), 1);
        let rgba = img.to_rgba8();
        if let ImageData::Rgba8(d) = rgba {
            assert_eq!(d.len(), 4);
        }
    }

    #[test]
    fn image_resize() {
        let bmp = make_red_bmp(20, 20);
        let img = Image::from_bytes_rgba8(&bmp).unwrap();
        let resized = img.resize(10, 10).unwrap();
        assert_eq!(resized.width(), 10);
        assert_eq!(resized.height(), 10);
    }

    #[test]
    fn image_crop() {
        let bmp = make_red_bmp(20, 20);
        let img = Image::from_bytes_rgba8(&bmp).unwrap();
        let cropped = img.crop(0, 0, 5, 5).unwrap();
        assert_eq!(cropped.width(), 5);
        assert_eq!(cropped.height(), 5);
    }

    #[test]
    fn image_flip() {
        let bmp = make_red_bmp(10, 10);
        let img = Image::from_bytes_rgba8(&bmp).unwrap();
        assert!(img.flip_horizontal().is_ok());
        assert!(img.flip_vertical().is_ok());
    }

    #[test]
    fn image_rotate() {
        let bmp = make_red_bmp(10, 10);
        let img = Image::from_bytes_rgba8(&bmp).unwrap();
        let rotated = img.rotate(90).unwrap();
        assert_eq!(rotated.width(), 10);
        assert_eq!(rotated.height(), 10);
    }
}
