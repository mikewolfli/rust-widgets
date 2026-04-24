//! PDF data types and structures.

use crate::core::{Rect, Size};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfSecurity {
    /// Optional password required to open the document.
    pub user_password: Option<String>,
    /// Optional owner password for privilege changes.
    pub owner_password: Option<String>,
    /// Whether printing is allowed.
    pub print_permission: bool,
    /// Whether content editing is allowed.
    pub edit_permission: bool,
    /// Whether content copy is allowed.
    pub copy_permission: bool,
    /// Whether annotations are allowed.
    pub annotation_permission: bool,
}
impl Default for PdfSecurity {
    fn default() -> Self {
        Self {
            user_password: None,
            owner_password: None,
            print_permission: true,
            edit_permission: true,
            copy_permission: true,
            annotation_permission: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PdfFormField {
    /// Text field with default value.
    TextField {
        name: String,
        rect: Rect,
        value: String,
    },
    /// Checkbox field.
    CheckBox {
        name: String,
        rect: Rect,
        checked: bool,
    },
    /// Button field.
    Button {
        name: String,
        rect: Rect,
        text: String,
    },
    ComboBox {
        name: String,
        rect: Rect,
        value: String,
        options: Vec<String>,
    },
    ListBox {
        name: String,
        rect: Rect,
        selected: Vec<usize>,
        options: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PdfPagination {
    /// Enable page-number footer output.
    pub(crate) enabled: bool,
    /// Prefix text before current/total numbers.
    pub(crate) prefix: String,
    /// One-based starting page number.
    pub(crate) start_at: u32,
    /// Distance from page right edge in points.
    pub(crate) right_margin: f32,
    /// Distance from page bottom edge in points.
    pub(crate) bottom_margin: f32,
    /// Font size for footer page-number text.
    pub(crate) font_size: f32,
}
impl Default for PdfPagination {
    fn default() -> Self {
        Self {
            enabled: false,
            prefix: "Page".to_string(),
            start_at: 1,
            right_margin: 140.0,
            bottom_margin: 20.0,
            font_size: 10.0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PdfFontResource {
    /// Resource key referenced by page content streams (e.g. F1).
    pub(crate) resource_name: String,
    /// PostScript-like base font name.
    pub(crate) base_font: String,
    /// Optional source path for embedded font diagnostics.
    pub(crate) source_path: Option<String>,
    /// Optional embedded font bytes.
    pub(crate) embedded_data: Vec<u8>,
}
impl PdfFontResource {
    pub(crate) fn core_helvetica(resource_name: &str) -> Self {
        Self {
            resource_name: resource_name.to_string(),
            base_font: "Helvetica".to_string(),
            source_path: None,
            embedded_data: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageEncodingRoute {
    ExactRgb,
    ExactRgbaDropAlpha,
    ExactGrayExpand,
    TruncatedOrPadded,
}
impl ImageEncodingRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ImageEncodingRoute::ExactRgb => "exact-rgb",
            ImageEncodingRoute::ExactRgbaDropAlpha => "exact-rgba-drop-alpha",
            ImageEncodingRoute::ExactGrayExpand => "exact-gray-expand",
            ImageEncodingRoute::TruncatedOrPadded => "raw-truncate-pad",
        }
    }
}
pub(crate) fn normalize_image_payload_to_rgb(
    image: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, ImageEncodingRoute) {
    let pixel_count = width.saturating_mul(height);
    let expected_rgb_len = pixel_count.saturating_mul(3);
    let expected_rgba_len = pixel_count.saturating_mul(4);
    let expected_gray_len = pixel_count;
    if image.len() == expected_rgb_len {
        return (image.to_vec(), ImageEncodingRoute::ExactRgb);
    }
    if image.len() == expected_rgba_len {
        let mut rgb = Vec::with_capacity(expected_rgb_len);
        for chunk in image.chunks_exact(4) {
            rgb.extend_from_slice(&chunk[..3]);
        }
        return (rgb, ImageEncodingRoute::ExactRgbaDropAlpha);
    }
    if image.len() == expected_gray_len {
        let mut rgb = Vec::with_capacity(expected_rgb_len);
        for gray in image {
            rgb.push(*gray);
            rgb.push(*gray);
            rgb.push(*gray);
        }
        return (rgb, ImageEncodingRoute::ExactGrayExpand);
    }
    let mut rgb = vec![0u8; expected_rgb_len];
    let copy_len = expected_rgb_len.min(image.len());
    rgb[..copy_len].copy_from_slice(&image[..copy_len]);
    (rgb, ImageEncodingRoute::TruncatedOrPadded)
}

pub(crate) struct ParsedPdfPage {
    pub(crate) size: Size,
    pub(crate) content: Vec<u8>,
}
