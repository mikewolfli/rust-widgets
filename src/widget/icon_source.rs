// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Lightweight, already-resolved icon content for widget presentation.

use crate::compat::String;
use crate::widget::display_widgets::icon::{IconData, IconName};
#[cfg(all(feature = "image", not(alloc_frugal)))]
use alloc::sync::Arc;

/// Persistable description of an icon source.
///
/// Unlike [`IconSource`], this contains no borrowed geometry or runtime pixel handle and can
/// therefore cross a document boundary. Resource descriptors must be resolved by an explicit
/// application context before they can be presented by a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconSpec {
    /// A built-in token shipped by this crate.
    Builtin(IconName),
    /// A project resource whose bytes are resolved by the host.
    Resource {
        /// Stable project-local resource identity.
        id: alloc::string::String,
        /// Whether the resource is a tintable monochrome mask or keeps its original colours.
        color_mode: IconColorMode,
    },
}

/// Color treatment declared for a persisted resource icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconColorMode {
    /// Treat the resource as a monochrome mask that may be tinted by the widget.
    Monochrome,
    /// Preserve the resource's own colors.
    Original,
}

/// Resolves project-scoped resource descriptions to already validated, drawable content.
///
/// Implementations own resource identity, permissions, loading policy, and cache lifetime. They
/// must not interpret an ID as a filesystem path unless that is the host's explicit resource
/// policy. Returning `IconSource` keeps IO and decoding outside widget setters and paint calls.
pub trait IconResourceResolver {
    /// Resolves one resource for the requested colour treatment.
    fn resolve_icon(
        &self,
        resource_id: &str,
        color_mode: IconColorMode,
    ) -> Result<IconSource, String>;
}

/// Immutable, validated RGBA8 pixels shareable by image-bearing controls.
///
/// Cloning this handle clones only the `Arc`; it never clones the pixel buffer. The image feature
/// and non-frugal profile gate match the existing shared image cache.
#[cfg(all(feature = "image", not(alloc_frugal)))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedIconPixels {
    pixels: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

#[cfg(all(feature = "image", not(alloc_frugal)))]
impl SharedIconPixels {
    /// Creates a shared handle by taking ownership of validated RGBA8 bytes.
    pub fn from_rgba8(pixels: Vec<u8>, width: u32, height: u32) -> Result<Self, String> {
        Self::from_shared_rgba8(Arc::new(pixels), width, height)
    }

    /// Creates a shared handle over an existing pixel allocation.
    ///
    /// This is the no-copy entry point for callers that already hold shared RGBA8 bytes, such as
    /// the image cache.
    pub fn from_shared_rgba8(
        pixels: Arc<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err(format!("shared icon dimensions must be non-zero, got {width}x{height}"));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixel_count| pixel_count.checked_mul(4))
            .ok_or_else(|| {
                format!("shared icon RGBA8 size overflows usize for {width}x{height}")
            })?;
        if pixels.len() != expected {
            return Err(format!(
                "shared icon has {} RGBA8 bytes but {width}x{height} requires {expected}",
                pixels.len()
            ));
        }
        Ok(Self { pixels, width, height })
    }

    /// Takes an image, moving RGBA8 bytes when possible and converting other formats once.
    pub fn from_image(image: crate::image::Image) -> Result<Self, String> {
        let width = image.width();
        let height = image.height();
        let pixels = match image.inner.data {
            crate::image::ImageData::Rgba8(pixels) => pixels,
            data => match data.to_rgba8(width, height) {
                crate::image::ImageData::Rgba8(pixels) => pixels,
                _ => return Err("image conversion did not produce RGBA8 pixels".to_string()),
            },
        };
        Self::from_rgba8(pixels, width, height)
    }

    /// Returns the source image width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the source image height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Borrows the shared RGBA8 pixel bytes.
    pub fn pixels(&self) -> &[u8] {
        self.pixels.as_slice()
    }

    /// Returns whether two handles reference the same pixel allocation.
    pub fn shares_allocation_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pixels, &other.pixels)
    }
}

/// An icon source that can be used without a resource resolver.
///
/// `Builtin` uses the crate's token and preserves its feature-dependent fallback. `StaticGeometry`
/// borrows caller-owned static path data directly and does not occupy the legacy name registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSource {
    /// One of the crate's built-in icon names.
    Builtin(IconName),
    /// Static path geometry on the grid described by its [`IconData`].
    StaticGeometry(&'static IconData),
}

/// Why an icon source could not be assigned to a collection item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconItemError {
    /// The requested item index does not exist.
    IndexOutOfBounds {
        /// The index the caller requested.
        index: usize,
        /// Number of items in the collection.
        len: usize,
    },
    /// No menu item with the requested stable identifier exists.
    ItemNotFound {
        /// The item identifier the caller requested.
        id: u64,
    },
    /// More than one menu item uses the requested identifier.
    AmbiguousItemId {
        /// The non-unique item identifier.
        id: u64,
    },
}

impl IconSource {
    /// Creates a source for a built-in token.
    pub const fn builtin(name: IconName) -> Self {
        Self::Builtin(name)
    }

    /// Creates a source borrowing static geometry.
    pub const fn static_geometry(data: &'static IconData) -> Self {
        Self::StaticGeometry(data)
    }
}

impl IconSpec {
    /// Resolves a persisted built-in source without any external services.
    ///
    /// A resource descriptor intentionally returns `None`: resolving it requires an explicit
    /// project/resource context and must not turn a resource ID into an assumed file path.
    pub fn builtin_source(&self) -> Option<IconSource> {
        match self {
            Self::Builtin(name) => Some(IconSource::Builtin(*name)),
            Self::Resource { .. } => None,
        }
    }
}

#[cfg(all(test, feature = "image", not(alloc_frugal)))]
mod shared_pixels_tests {
    use super::SharedIconPixels;
    use crate::image::Image;

    #[test]
    fn owned_rgba_image_transfers_its_pixel_allocation() {
        let bytes = vec![7; 2 * 3 * 4];
        let original_ptr = bytes.as_ptr();
        let image = Image::from_rgba(bytes, 2, 3);
        let shared = SharedIconPixels::from_image(image).expect("valid RGBA image");

        assert_eq!(shared.width(), 2);
        assert_eq!(shared.height(), 3);
        assert_eq!(shared.pixels().as_ptr(), original_ptr, "RGBA bytes must be moved, not copied");
        let clone = shared.clone();
        assert!(shared.shares_allocation_with(&clone));
    }

    #[test]
    fn shared_rgba_constructor_rejects_invalid_dimensions_and_payloads() {
        assert!(SharedIconPixels::from_rgba8(Vec::new(), 0, 1).is_err());
        assert!(SharedIconPixels::from_rgba8(vec![0; 3], 1, 1).is_err());
        assert!(SharedIconPixels::from_rgba8(Vec::new(), u32::MAX, u32::MAX).is_err());
    }
}
