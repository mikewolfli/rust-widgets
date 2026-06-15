//! Image format enumeration with all mainstream formats.

/// All supported image formats for decoding and encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// Unknown/unrecognized format.
    Unknown,
    /// Raw RGBA pixel data (8 bits per channel).
    Rgba8,
    /// Raw RGB pixel data (8 bits per channel).
    Rgb8,
    /// Portable Network Graphics.
    Png,
    /// Joint Photographic Experts Group.
    Jpeg,
    /// Bitmap.
    Bmp,
    /// Graphics Interchange Format (static and animated).
    Gif,
    /// WebP (both lossy and lossless).
    WebP,
    /// Tagged Image File Format.
    Tiff,
    /// AV1 Image File Format.
    Avif,
    /// ICO (Windows icon).
    Ico,
    /// Netpbm (PBM/PGM/PPM/PAM).
    Pnm,
    /// Quite OK Image format.
    Qoi,
    /// Farbfeld (lossless image format).
    Farbfeld,
    /// Scalable Vector Graphics.
    Svg,
    /// Compressed SVG (.svgz).
    Svgz,
}

impl ImageFormat {
    /// Returns the MIME type string for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Unknown => "application/octet-stream",
            ImageFormat::Rgba8 => "image/x-rgba",
            ImageFormat::Rgb8 => "image/x-rgb",
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Pnm => "image/x-portable-anymap",
            ImageFormat::Qoi => "image/x-qoi",
            ImageFormat::Farbfeld => "image/x-farbfeld",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Svgz => "image/svg+xml-compressed",
        }
    }

    /// Returns the common file extension (without dot) for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Unknown => "bin",
            ImageFormat::Rgba8 => "rgba",
            ImageFormat::Rgb8 => "rgb",
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Gif => "gif",
            ImageFormat::WebP => "webp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Avif => "avif",
            ImageFormat::Ico => "ico",
            ImageFormat::Pnm => "pnm",
            ImageFormat::Qoi => "qoi",
            ImageFormat::Farbfeld => "ff",
            ImageFormat::Svg => "svg",
            ImageFormat::Svgz => "svgz",
        }
    }

    /// Returns true if this format supports animation (multiple frames).
    pub fn supports_animation(&self) -> bool {
        matches!(self, ImageFormat::Gif | ImageFormat::WebP | ImageFormat::Avif | ImageFormat::Png)
    }

    /// Returns true if this format supports lossy compression.
    pub fn supports_lossy(&self) -> bool {
        matches!(self, ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Avif)
    }

    /// Returns true if this format supports transparency/alpha channel.
    pub fn supports_alpha(&self) -> bool {
        !matches!(self, ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Pnm | ImageFormat::Rgb8)
    }

    /// Returns true if this is a vector format (SVG/SVGZ).
    pub fn is_vector(&self) -> bool {
        matches!(self, ImageFormat::Svg | ImageFormat::Svgz)
    }
}

impl Default for ImageFormat {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Pixel data representation variants.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageData {
    /// 8-bit RGBA pixels (4 bytes per pixel).
    Rgba8(Vec<u8>),
    /// 8-bit RGB pixels (3 bytes per pixel).
    Rgb8(Vec<u8>),
    /// 8-bit grayscale (1 byte per pixel).
    Grayscale8(Vec<u8>),
    /// 16-bit grayscale (2 bytes per pixel, big-endian).
    Grayscale16(Vec<u8>),
    /// 16-bit RGBA pixels (8 bytes per pixel, big-endian per channel).
    Rgba16(Vec<u8>),
    /// 16-bit RGB pixels (6 bytes per pixel, big-endian per channel).
    Rgb16(Vec<u8>),
}

impl ImageData {
    /// Returns the raw pixel bytes.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ImageData::Rgba8(d) => d,
            ImageData::Rgb8(d) => d,
            ImageData::Grayscale8(d) => d,
            ImageData::Grayscale16(d) => d,
            ImageData::Rgba16(d) => d,
            ImageData::Rgb16(d) => d,
        }
    }

    /// Consumes self and returns the raw pixel bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            ImageData::Rgba8(d) => d,
            ImageData::Rgb8(d) => d,
            ImageData::Grayscale8(d) => d,
            ImageData::Grayscale16(d) => d,
            ImageData::Rgba16(d) => d,
            ImageData::Rgb16(d) => d,
        }
    }

    /// Returns the number of bytes per pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            ImageData::Rgba8(_) => 4,
            ImageData::Rgb8(_) => 3,
            ImageData::Grayscale8(_) => 1,
            ImageData::Grayscale16(_) => 2,
            ImageData::Rgba16(_) => 8,
            ImageData::Rgb16(_) => 6,
        }
    }

    /// Returns the number of channels.
    pub fn channels(&self) -> usize {
        match self {
            ImageData::Rgba8(_) | ImageData::Rgba16(_) => 4,
            ImageData::Rgb8(_) | ImageData::Rgb16(_) => 3,
            ImageData::Grayscale8(_) | ImageData::Grayscale16(_) => 1,
        }
    }

    /// Converts to RGBA8 if not already. Returns self if already RGBA8.
    pub fn to_rgba8(&self, width: u32, height: u32) -> ImageData {
        if matches!(self, ImageData::Rgba8(_)) {
            return self.clone();
        }
        let total = (width * height) as usize;
        match self {
            ImageData::Rgb8(d) => {
                let mut rgba = Vec::with_capacity(total * 4);
                for chunk in d.chunks(3) {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                    rgba.push(chunk[2]);
                    rgba.push(255);
                }
                ImageData::Rgba8(rgba)
            }
            ImageData::Grayscale8(d) => {
                let mut rgba = Vec::with_capacity(total * 4);
                for &g in d {
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(255);
                }
                ImageData::Rgba8(rgba)
            }
            ImageData::Grayscale16(d) => {
                let mut rgba = Vec::with_capacity(total * 4);
                for chunk in d.chunks(2) {
                    let g = (u16::from_be_bytes([chunk[0], chunk[1]]) >> 8) as u8;
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(255);
                }
                ImageData::Rgba8(rgba)
            }
            ImageData::Rgba16(d) => {
                let mut rgba = Vec::with_capacity(total * 4);
                for chunk in d.chunks(8) {
                    rgba.push((u16::from_be_bytes([chunk[0], chunk[1]]) >> 8) as u8);
                    rgba.push((u16::from_be_bytes([chunk[2], chunk[3]]) >> 8) as u8);
                    rgba.push((u16::from_be_bytes([chunk[4], chunk[5]]) >> 8) as u8);
                    rgba.push((u16::from_be_bytes([chunk[6], chunk[7]]) >> 8) as u8);
                }
                ImageData::Rgba8(rgba)
            }
            ImageData::Rgb16(d) => {
                let mut rgba = Vec::with_capacity(total * 4);
                for chunk in d.chunks(6) {
                    rgba.push((u16::from_be_bytes([chunk[0], chunk[1]]) >> 8) as u8);
                    rgba.push((u16::from_be_bytes([chunk[2], chunk[3]]) >> 8) as u8);
                    rgba.push((u16::from_be_bytes([chunk[4], chunk[5]]) >> 8) as u8);
                    rgba.push(255);
                }
                ImageData::Rgba8(rgba)
            }
            ImageData::Rgba8(_) => unreachable!(), // Caught by matches! check above
        }
    }
}

/// EXIF data extracted from image files.
#[derive(Debug, Clone, Default)]
pub struct ExifData {
    /// Camera make.
    pub make: String,
    /// Camera model.
    pub model: String,
    /// ISO speed rating.
    pub iso: Option<u32>,
    /// Focal length in millimeters.
    pub focal_length: Option<f64>,
    /// Aperture (F-number).
    pub aperture: Option<f64>,
    /// Exposure time in seconds.
    pub exposure_time: Option<f64>,
    /// Original date/time.
    pub date_time: Option<String>,
    /// GPS latitude in decimal degrees.
    pub gps_latitude: Option<f64>,
    /// GPS longitude in decimal degrees.
    pub gps_longitude: Option<f64>,
    /// Image width from EXIF.
    pub exif_width: Option<u32>,
    /// Image height from EXIF.
    pub exif_height: Option<u32>,
    /// Orientation (1 = normal, 3 = 180 rotated, 6 = 90 CW, 8 = 90 CCW).
    pub orientation: Option<u8>,
}

/// Parsed color space information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// sRGB (default).
    Srgb,
    /// Adobe RGB.
    AdobeRgb,
    /// Linear RGB.
    LinearRgb,
    /// ProPhoto RGB.
    ProPhotoRgb,
    /// Display P3.
    DisplayP3,
    /// Grayscale.
    Grayscale,
    /// CMYK.
    Cmyk,
    /// Unknown color space.
    Unknown,
}

impl Default for ColorSpace {
    fn default() -> Self {
        Self::Srgb
    }
}

/// Decoded image with metadata.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// The image format as detected.
    pub format: ImageFormat,
    /// Pixel data.
    pub data: ImageData,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// EXIF metadata (if available).
    pub exif: ExifData,
    /// Color space information.
    pub color_space: ColorSpace,
}

impl DecodedImage {
    /// Create a new decoded image.
    pub fn new(format: ImageFormat, data: ImageData, width: u32, height: u32) -> Self {
        Self {
            format,
            data,
            width,
            height,
            exif: ExifData::default(),
            color_space: ColorSpace::Srgb,
        }
    }

    /// Returns the total number of pixels.
    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Returns a reference to the RGBA8 pixel data, converting if needed.
    pub fn as_rgba8(&self) -> ImageData {
        self.data.to_rgba8(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_format_mime_types() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
        assert_eq!(ImageFormat::Svg.mime_type(), "image/svg+xml");
        assert_eq!(ImageFormat::Svgz.mime_type(), "image/svg+xml-compressed");
    }

    #[test]
    fn image_format_extensions() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Avif.extension(), "avif");
        assert_eq!(ImageFormat::Ico.extension(), "ico");
        assert_eq!(ImageFormat::Farbfeld.extension(), "ff");
    }

    #[test]
    fn image_format_supports_checks() {
        assert!(ImageFormat::Png.supports_animation());
        assert!(!ImageFormat::Jpeg.supports_animation());
        assert!(ImageFormat::Jpeg.supports_lossy());
        assert!(!ImageFormat::Png.supports_lossy());
        assert!(ImageFormat::Png.supports_alpha());
        assert!(!ImageFormat::Jpeg.supports_alpha());
        assert!(ImageFormat::Svg.is_vector());
        assert!(ImageFormat::Svgz.is_vector());
        assert!(!ImageFormat::Png.is_vector());
    }

    #[test]
    fn image_data_rgb8_to_rgba8() {
        let rgb = ImageData::Rgb8(vec![255, 0, 0, 0, 255, 0]);
        let rgba = rgb.to_rgba8(2, 1);
        if let ImageData::Rgba8(d) = rgba {
            assert_eq!(d.len(), 8);
            assert_eq!(&d[0..4], &[255, 0, 0, 255]);
            assert_eq!(&d[4..8], &[0, 255, 0, 255]);
        } else {
            panic!("Expected Rgba8");
        }
    }

    #[test]
    fn image_data_grayscale8_to_rgba8() {
        let g = ImageData::Grayscale8(vec![128, 64]);
        let rgba = g.to_rgba8(2, 1);
        if let ImageData::Rgba8(d) = rgba {
            assert_eq!(&d[0..4], &[128, 128, 128, 255]);
            assert_eq!(&d[4..8], &[64, 64, 64, 255]);
        } else {
            panic!("Expected Rgba8");
        }
    }

    #[test]
    fn decoded_image_pixel_count() {
        let img = DecodedImage::new(ImageFormat::Rgba8, ImageData::Rgba8(vec![0; 400]), 20, 20);
        assert_eq!(img.pixel_count(), 400);
    }

    #[test]
    fn image_data_bytes_per_pixel() {
        assert_eq!(ImageData::Rgba8(vec![]).bytes_per_pixel(), 4);
        assert_eq!(ImageData::Rgb8(vec![]).bytes_per_pixel(), 3);
        assert_eq!(ImageData::Grayscale8(vec![]).bytes_per_pixel(), 1);
        assert_eq!(ImageData::Grayscale16(vec![]).bytes_per_pixel(), 2);
    }
}
