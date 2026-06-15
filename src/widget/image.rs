//! Image structure for widget icons and favicons.

/// Image pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Unknown/unspecified format.
    Unknown,
    /// Raw RGBA pixel data (8 bits per channel).
    Rgba8,
    /// Raw RGB pixel data (8 bits per channel).
    Rgb8,
    /// PNG encoded image.
    Png,
    /// JPEG encoded image.
    Jpeg,
    /// BMP encoded image.
    Bmp,
}

/// Image structure for widget icons and favicons.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}
impl Image {
    /// Creates an empty image.
    pub fn new() -> Self {
        Self { data: Vec::new(), format: ImageFormat::Unknown, width: 0, height: 0 }
    }

    /// Creates an image from raw RGBA data.
    pub fn from_rgba(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self { data, format: ImageFormat::Rgba8, width, height }
    }

    /// Returns the image width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the image height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the image format.
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// Returns whether the image has pixel data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the raw pixel data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Decode image from raw bytes.
    ///
    /// Supports PNG, JPEG, BMP, and other common formats via the embedded
    /// image decoder (miniz_oxide for PNG decompression).
    ///
    /// Returns an `Image` with RGBA8 pixel data ready for rendering.
    #[cfg(feature = "image")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let decoded = crate::image::decoder::decode_to_rgba8(data)?;
        let pixel_data = match decoded.data {
            crate::image::format::ImageData::Rgba8(d) => d,
            _ => return Err("Decoded image is not RGBA8".to_string()),
        };
        Ok(Self {
            data: pixel_data,
            format: ImageFormat::Rgba8,
            width: decoded.width,
            height: decoded.height,
        })
    }

    /// Load an image from a file path.
    ///
    /// Supports PNG, JPEG, BMP, and other common formats.
    /// The image is decoded into RGBA8 pixel data for rendering.
    #[cfg(feature = "image")]
    pub fn from_file(path: &str) -> Result<Self, String> {
        use std::io::Read;
        let mut file = std::fs::File::open(path)
            .map_err(|e| format!("Failed to open image file '{}': {}", path, e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read image file '{}': {}", path, e))?;
        Self::from_bytes(&data)
    }
}
impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}
