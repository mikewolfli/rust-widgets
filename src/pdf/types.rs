// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF data types and structures.

use crate::core::{Rect, Size};
/// Permission and password settings applied to a generated PDF.
///
/// The `*_permission` flags are the crate's declared intent; whether they are
/// enforced depends on the consumer applying them as PDF encryption
/// permissions. All four default to `true`, so a [`PdfSecurity::default`] grants
/// everything and sets no passwords.
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

/// One interactive form field placed on a page.
///
/// The `rect` of each variant is in PDF user-space **points** (1/72 inch),
/// unlike widget geometry elsewhere in the crate which is in logical pixels.
#[derive(Debug, Clone)]
pub enum PdfFormField {
    /// Text field with default value.
    TextField {
        /// Field name, unique within the form.
        name: String,
        /// Position and extent in PDF points.
        rect: Rect,
        /// Initial contents of the field.
        value: String,
    },
    /// Checkbox field.
    CheckBox {
        /// Field name, unique within the form.
        name: String,
        /// Position and extent in PDF points.
        rect: Rect,
        /// Initial tick state.
        checked: bool,
    },
    /// Button field.
    Button {
        /// Field name, unique within the form.
        name: String,
        /// Position and extent in PDF points.
        rect: Rect,
        /// Caption drawn on the button face.
        text: String,
    },
    /// Drop-down list permitting exactly one selection.
    ComboBox {
        /// Field name, unique within the form.
        name: String,
        /// Position and extent in PDF points.
        rect: Rect,
        /// Currently selected option text; should match one entry of `options`,
        /// though nothing here enforces that.
        value: String,
        /// Selectable entries, in display order.
        options: Vec<String>,
    },
    /// Scrollable list permitting zero or more selections.
    ListBox {
        /// Field name, unique within the form.
        name: String,
        /// Position and extent in PDF points.
        rect: Rect,
        /// Zero-based indices into `options`. Out-of-range indices are the
        /// caller's responsibility; they are not validated here.
        selected: Vec<usize>,
        /// Selectable entries, in display order.
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
    Rgb,
    RgbaDropAlpha,
    GrayExpand,
}
impl ImageEncodingRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ImageEncodingRoute::Rgb => "exact-rgb",
            ImageEncodingRoute::RgbaDropAlpha => "exact-rgba-drop-alpha",
            ImageEncodingRoute::GrayExpand => "exact-gray-expand",
        }
    }
}
pub(crate) fn normalize_image_payload_to_rgb(
    image: &[u8],
    width: usize,
    height: usize,
) -> Result<(Vec<u8>, ImageEncodingRoute), String> {
    let pixel_count = width.checked_mul(height).ok_or_else(|| {
        format!("PDF image dimensions {width}x{height} overflow addressable size")
    })?;
    let expected_rgb_len = pixel_count.checked_mul(3).ok_or_else(|| {
        format!("PDF RGB image dimensions {width}x{height} overflow addressable size")
    })?;
    let expected_rgba_len = pixel_count.checked_mul(4).ok_or_else(|| {
        format!("PDF RGBA image dimensions {width}x{height} overflow addressable size")
    })?;
    let expected_gray_len = pixel_count;
    if image.len() == expected_rgb_len {
        let mut rgb = reserve_rgb(expected_rgb_len)?;
        rgb.extend_from_slice(image);
        return Ok((rgb, ImageEncodingRoute::Rgb));
    }
    if image.len() == expected_rgba_len {
        let mut rgb = reserve_rgb(expected_rgb_len)?;
        for chunk in image.as_chunks::<4>().0 {
            rgb.extend_from_slice(&chunk[..3]);
        }
        return Ok((rgb, ImageEncodingRoute::RgbaDropAlpha));
    }
    if image.len() == expected_gray_len {
        let mut rgb = reserve_rgb(expected_rgb_len)?;
        for gray in image {
            rgb.push(*gray);
            rgb.push(*gray);
            rgb.push(*gray);
        }
        return Ok((rgb, ImageEncodingRoute::GrayExpand));
    }
    Err(format!(
        "PDF image payload has {} byte(s), but {width}x{height} requires {expected_rgb_len} (RGB), \
         {expected_rgba_len} (RGBA), or {expected_gray_len} (grayscale); refusing to alter pixels",
        image.len()
    ))
}

fn reserve_rgb(length: usize) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(length)
        .map_err(|error| format!("unable to allocate {length} bytes for PDF image: {error}"))?;
    Ok(rgb)
}

pub(crate) struct ParsedPdfPage {
    pub(crate) size: Size,
    pub(crate) content: Vec<u8>,
}
