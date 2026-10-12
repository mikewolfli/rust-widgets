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

    /// Wraps the shared pixels the decode cache already holds for `data`.
    ///
    /// This is the no-copy path from a decoded resource to a drawable handle: the cache's `Arc` is
    /// reused, so the only work is wrapping it with the extent the cache stored beside the bytes.
    /// `None` means the bytes did not decode.
    pub fn from_cached_bytes(data: &[u8]) -> Option<Self> {
        Self::from_cached(crate::image::cache::pixels_rgba8_sized_or_none(data)?)
    }

    /// Wraps the shared pixels the decode cache already holds for the file at `path`.
    ///
    /// See [`Self::from_cached_bytes`]; a missing or undecodable file is reported by the cache and
    /// yields `None`, exactly as [`crate::image::cache::file_rgba8_or_none`] does.
    pub fn from_cached_file(path: impl AsRef<std::path::Path>) -> Option<Self> {
        Self::from_cached(crate::image::cache::file_rgba8_sized_or_none(path)?)
    }

    /// Validates a cache entry's pixels and extent into a handle.
    ///
    /// The cache produces both together, so a rejection here would mean the cache is inconsistent
    /// with itself — reported rather than discarded, because a silent `None` would look like
    /// "the file was missing".
    fn from_cached((pixels, width, height): (Arc<Vec<u8>>, u32, u32)) -> Option<Self> {
        match Self::from_shared_rgba8(pixels, width, height) {
            Ok(handle) => Some(handle),
            Err(error) => {
                log::error!("decode cache produced an invalid shared icon ({error})");
                None
            }
        }
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

    /// The box this image occupies inside `slot` with its **aspect ratio preserved**.
    ///
    /// `contain`, not `stretch`: the image is scaled uniformly to the largest size that fits and
    /// centred in the slot. A square source in a square slot therefore gets exactly the slot back —
    /// byte-identical to stretching it — while a non-square source is no longer distorted, which is
    /// what the slot's own reserved geometry would otherwise do to it.
    pub fn fit_into(&self, slot: crate::core::Rect) -> crate::core::Rect {
        fit_image_into(self.width, self.height, slot)
    }

    /// Returns whether two handles reference the same pixel allocation.
    pub fn shares_allocation_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pixels, &other.pixels)
    }
}

/// The box a `source_width x source_height` image occupies inside `slot`, with its aspect ratio
/// preserved and centred.
///
/// One fit rule for every image this crate draws into an icon slot: a square source in a square slot
/// gets the slot back unchanged, and a non-square source is fitted rather than distorted. Shared by
/// the [`SharedIconPixels`] handle and by the legacy `Image` slots so the two cannot disagree about
/// what "draw this image here" means.
pub fn fit_image_into(
    source_width: u32,
    source_height: u32,
    slot: crate::core::Rect,
) -> crate::core::Rect {
    if slot.width == 0 || slot.height == 0 || source_width == 0 || source_height == 0 {
        return slot;
    }
    let scale =
        (slot.width as f32 / source_width as f32).min(slot.height as f32 / source_height as f32);
    let width = ((source_width as f32 * scale).round() as u32).clamp(1, slot.width);
    let height = ((source_height as f32 * scale).round() as u32).clamp(1, slot.height);
    crate::core::Rect::new(
        slot.x + (slot.width - width) as i32 / 2,
        slot.y + (slot.height - height) as i32 / 2,
        width,
        height,
    )
}

/// Normalises a legacy widget icon image to RGBA8 once, at assignment time.
/// Widgets that keep the old `Option<Image>` icon next to a typed [`IconSource`] paint it by
/// borrowing the bytes, and a per-frame transcode of a non-RGBA8 image would be work that cannot
/// change between frames. An image that is already RGBA8 is returned untouched, so the common
/// [`Image::from_rgba`](crate::image::Image::from_rgba) constructor never copies.
#[cfg(feature = "image")]
pub(crate) fn normalize_rgba8_image(image: crate::image::Image) -> crate::image::Image {
    let mut image = image;
    if !matches!(image.inner.data, crate::image::ImageData::Rgba8(_)) {
        let width = image.width();
        let height = image.height();
        image.inner.data = image.inner.data.to_rgba8(width, height);
    }
    image
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// No item with the requested **string** identifier exists.
    ///
    /// A second pair rather than a widened payload because the crate really does have two identity
    /// kinds: `MenuItem` ids are numbers, while a chip's id is host-supplied text. Widening the
    /// numeric payload to a string would make every existing caller convert an id it already has
    /// the right type for.
    UnknownItemId {
        /// The item identifier the caller requested.
        id: crate::compat::String,
    },
    /// More than one item uses the requested string identifier.
    DuplicateItemId {
        /// The non-unique item identifier.
        id: crate::compat::String,
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

    /// A 4x2 PNG, encoded through the crate's own encoder so the fixture is a real decode input.
    fn non_square_png() -> Vec<u8> {
        use crate::image::format::{DecodedImage, ImageData, ImageFormat};
        let decoded =
            DecodedImage::new(ImageFormat::Rgba8, ImageData::Rgba8(vec![9; 4 * 2 * 4]), 4, 2);
        crate::image::encode(&decoded, ImageFormat::Png).expect("the encoder produces a PNG")
    }

    #[test]
    fn cached_pixels_become_a_sized_handle_without_copying() {
        let png = non_square_png();
        let first = SharedIconPixels::from_cached_bytes(&png).expect("the PNG decodes");
        assert_eq!((first.width(), first.height()), (4, 2), "the decoded extent is kept");

        // The second request is answered by the cache entry the first one inserted, so the two
        // handles must share one allocation: this is the seam that stops a control from paying a
        // copy (plus a decode) for pixels the cache already holds.
        let second = SharedIconPixels::from_cached_bytes(&png).expect("the PNG is cached");
        assert!(first.shares_allocation_with(&second));
        assert_eq!(first.pixels().len(), 4 * 2 * 4);

        assert!(
            SharedIconPixels::from_cached_bytes(b"not an image at all").is_none(),
            "undecodable bytes produce no handle"
        );
    }

    #[test]
    fn fit_into_preserves_the_aspect_ratio_and_centres_the_image() {
        use crate::core::Rect;
        let wide = SharedIconPixels::from_rgba8(vec![0; 8 * 4 * 4], 8, 4).expect("valid RGBA");

        // A square slot: the wide image keeps 2:1 and is centred vertically, never stretched.
        let slot = Rect::new(10, 20, 20, 20);
        let fitted = wide.fit_into(slot);
        assert_eq!((fitted.width, fitted.height), (20, 10));
        assert_eq!((fitted.x, fitted.y), (10, 25));

        // A square image in a square slot gets the slot back byte-for-byte, which is what makes
        // this a no-op for every existing icon.
        let square = SharedIconPixels::from_rgba8(vec![0; 4 * 4 * 4], 4, 4).expect("valid RGBA");
        assert_eq!(square.fit_into(slot), slot);

        // A degenerate slot is returned untouched rather than divided by zero.
        assert_eq!(square.fit_into(Rect::new(0, 0, 0, 0)), Rect::new(0, 0, 0, 0));
    }
}
