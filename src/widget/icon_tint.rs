// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One control's icon tint: the colour, and the composed copy image icons need.
//!
//! # Why this is shared instead of per control
//!
//! Two controls publish an independent icon colour (`Button` and `ToolButton`). The rule for what
//! "tint an icon" means — a vector icon is painted in the colour; an image is treated as a
//! monochrome mask, keeping its alpha and taking the tint's RGB — is one rule, and so is the cache
//! that keeps it from being recomputed on every repaint. Writing it twice would give the two
//! controls a way to disagree about either (principle #54/#101).
//!
//! # What the cache is keyed by
//!
//! `(tint, source identity)`: a pixel handle is compared by allocation identity (the same sharing
//! rule the decode cache relies on), and a legacy [`Image`] by the address and length of its RGBA8
//! bytes. Every setter that can change a control's icon calls [`IconTint::clear`], so the cache can
//! never outlive the source it was composed from; the identity check is the second line of defence
//! for a caller that swaps the handle without going through those setters.
//!
//! # Profiles
//!
//! The colour part (`tint`, `color_or`, `set`) exists in every profile, because a vector icon is
//! drawn from geometry rather than from pixels. The composition half exists only where the image
//! feature is compiled: a frugal build has no `SharedIconPixels` to compose.

use crate::core::Color;

#[cfg(all(feature = "image", not(alloc_frugal)))]
use crate::widget::icon_source::SharedIconPixels;

/// The tint colour of one control's icon, with the composed copy for image icons.
#[derive(Debug, Default)]
pub struct IconTint {
    /// `None` means "no explicit colour": the caller keeps whatever ink it would have used.
    tint: Option<Color>,
    /// The composed copy of the current image, and the inputs that produced it.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    composed: Option<Composed>,
}

/// A tinted copy of one icon, with what produced it.
#[cfg(all(feature = "image", not(alloc_frugal)))]
#[derive(Debug)]
struct Composed {
    /// The tint applied to the source (`None` for a resampled-but-untinted copy).
    tint: Option<Color>,
    /// The source the copy was made from.
    source: ComposedSource,
    /// The tinted result, shared like any other icon handle.
    tinted: SharedIconPixels,
}

/// Identity of the source a [`Composed`] copy came from.
#[cfg(all(feature = "image", not(alloc_frugal)))]
#[derive(Debug)]
enum ComposedSource {
    /// A shared pixel handle, compared by allocation identity.
    Pixels(SharedIconPixels),
    /// A legacy image, compared by the address and length of its RGBA8 bytes and the box it was
    /// resampled into.
    Image {
        /// Address of the source bytes, or `0` when the source needed converting first.
        ptr: usize,
        /// Byte length of the source.
        len: usize,
        /// Width of the box the copy was resampled into.
        width: u32,
        /// Height of the box the copy was resampled into.
        height: u32,
    },
}

impl IconTint {
    /// A control with no tint.
    pub const fn new() -> Self {
        Self {
            tint: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            composed: None,
        }
    }

    /// The explicit tint colour, or `None` when the icon follows the control's own ink.
    pub fn tint(&self) -> Option<Color> {
        self.tint
    }

    /// The colour to paint a vector icon with: the tint when set, else `fallback`.
    pub fn color_or(&self, fallback: Color) -> Color {
        self.tint.unwrap_or(fallback)
    }

    /// Sets or clears the tint, reporting whether it changed.
    ///
    /// A change drops the composed copy: it was built for the previous colour, and drawing it
    /// would show a stale tint until something else invalidated it.
    pub fn set(&mut self, tint: Option<Color>) -> bool {
        if self.tint == tint {
            return false;
        }
        self.tint = tint;
        self.clear();
        true
    }

    /// Drops the composed copy.
    ///
    /// A control calls this from every setter that changes its icon (path, typed source, shared
    /// pixels, image), so a copy can never be drawn against a source it was not made from.
    pub fn clear(&mut self) {
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.composed = None;
        }
    }

    /// The tinted copy of `source`, or `None` when there is no tint to apply.
    ///
    /// `None` is the caller's signal to draw `source` unchanged, which keeps the untinted path
    /// byte-identical to what it drew before the tint existed.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn compose_pixels(&mut self, source: &SharedIconPixels) -> Option<SharedIconPixels> {
        let tint = self.tint?;
        if let Some(composed) = self.composed.as_ref() {
            if composed.tint == self.tint {
                if let ComposedSource::Pixels(previous) = &composed.source {
                    if previous.shares_allocation_with(source) {
                        return Some(composed.tinted.clone());
                    }
                }
            }
        }
        let tinted = source.tinted(tint);
        self.composed = Some(Composed {
            tint: self.tint,
            source: ComposedSource::Pixels(source.clone()),
            tinted: tinted.clone(),
        });
        Some(tinted)
    }

    /// A legacy image prepared for the icon box: resampled to `target`, and tinted when a tint is
    /// set.
    ///
    /// # Why this resamples rather than handing the bytes back
    ///
    /// `RenderContext::draw_image` blits a buffer **whose dimensions are the rectangle's**: an
    /// image drawn into a larger box is refused (nothing is painted) and one drawn into a smaller
    /// box is cropped. A control that reserves a slot for an icon therefore has to hand the paint
    /// a buffer that matches that slot, which is what this does. The resample is cached per
    /// `(tint, source, target)`, so a repaint costs a handle clone.
    ///
    /// `None` means "draw the source as it is": no tint **and** the source already matches the
    /// target. That keeps the common case (an icon decoded at exactly its slot size) free of any
    /// copy at all, and it keeps the untinted path byte-identical to what it drew before.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn compose_image(
        &mut self,
        image: &crate::image::Image,
        target: (u32, u32),
    ) -> Option<SharedIconPixels> {
        let (width, height) = target;
        let (src_width, src_height) = (image.width(), image.height());
        if width == 0 || height == 0 || src_width == 0 || src_height == 0 {
            return None;
        }
        if self.tint.is_none() && (width, height) == (src_width, src_height) {
            return None;
        }

        let bytes = image.rgba8_data();
        let key = (bytes.map_or(0, |data| data.as_ptr() as usize), bytes.map_or(0, <[u8]>::len));
        if let Some(composed) = self.composed.as_ref() {
            if composed.tint == self.tint {
                if let ComposedSource::Image { ptr, len, width: w, height: h } = composed.source {
                    if (ptr, len, w, h) == (key.0, key.1, width, height) {
                        return Some(composed.tinted.clone());
                    }
                }
            }
        }

        // A source that is not RGBA8 is converted once per call: a converted buffer has no stable
        // identity to cache against, and decoded icons are RGBA8 in practice.
        let converted;
        let source_bytes = match bytes {
            Some(data) => data,
            None => {
                let crate::image::ImageData::Rgba8(data) = image.to_rgba8() else {
                    return None;
                };
                converted = data;
                converted.as_slice()
            }
        };
        let resized = crate::image::transform::resize_rgba8(
            source_bytes,
            src_width,
            src_height,
            width,
            height,
        )
        .ok()?;
        let tinted = match self.tint {
            Some(tint) => SharedIconPixels::tinted_rgba8(&resized, width, height, tint)?,
            None => SharedIconPixels::from_rgba8(resized, width, height).ok()?,
        };
        self.composed = Some(Composed {
            tint: self.tint,
            source: ComposedSource::Image { ptr: key.0, len: key.1, width, height },
            tinted: tinted.clone(),
        });
        Some(tinted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    use crate::compat::Vec;

    /// With no tint the caller keeps its own ink and the source is drawn unchanged.
    #[test]
    fn no_tint_keeps_the_callers_ink() {
        let tint = IconTint::new();
        assert_eq!(tint.tint(), None);
        assert_eq!(tint.color_or(Color::BLACK), Color::BLACK);
    }

    /// Setting a colour changes what a vector icon is painted with, and the same colour is a no-op.
    #[test]
    fn setting_a_colour_reports_the_change_and_feeds_the_vector_path() {
        let mut tint = IconTint::new();
        assert!(tint.set(Some(Color::RED)));
        assert_eq!(tint.color_or(Color::BLACK), Color::RED);
        assert!(!tint.set(Some(Color::RED)), "an unchanged tint must not report a change");
        assert!(tint.set(None));
        assert_eq!(tint.color_or(Color::BLACK), Color::BLACK);
    }

    /// An image source composes once per `(tint, source)`, and a tint change recomposes.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn composing_an_image_source_is_cached_per_tint_and_source() {
        let source =
            SharedIconPixels::from_rgba8(vec![255, 255, 255, 255, 255, 255, 255, 128], 2, 1)
                .expect("valid RGBA8 data");
        let mut tint = IconTint::new();
        assert!(tint.compose_pixels(&source).is_none(), "no tint means no copy");

        tint.set(Some(Color::rgba(10, 20, 30, 255)));
        let first = tint.compose_pixels(&source).expect("a tinted copy");
        assert_eq!(first.pixels(), &[10, 20, 30, 255, 10, 20, 30, 128]);
        let second = tint.compose_pixels(&source).expect("still tinted");
        assert!(
            first.shares_allocation_with(&second),
            "the same source and tint must reuse the composed copy"
        );

        tint.set(Some(Color::rgba(40, 50, 60, 255)));
        let recoloured = tint.compose_pixels(&source).expect("recomposed");
        assert_eq!(recoloured.pixels()[0], 40);
        assert!(
            !recoloured.shares_allocation_with(&first),
            "a new tint must not reuse the previous composition"
        );

        // A different source must not be served from the cache of the previous one.
        let other = SharedIconPixels::from_rgba8(vec![9, 9, 9, 255], 1, 1).expect("valid");
        let from_other = tint.compose_pixels(&other).expect("composed from the new source");
        assert_eq!(from_other.pixels(), &[40, 50, 60, 255]);
        assert!(!from_other.shares_allocation_with(&recoloured));
    }

    /// A legacy image is tinted into a handle, and the image path keeps its own dimensions.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn composing_a_legacy_image_produces_a_tinted_handle() {
        let image = crate::image::Image::from_raw(
            crate::image::ImageData::Rgba8(Vec::from([255u8, 255, 255, 200])),
            1,
            1,
        );
        let mut tint = IconTint::new();
        assert!(
            tint.compose_image(&image, (1, 1)).is_none(),
            "an untinted image that already matches its box is drawn as it is"
        );

        // A box larger than the source is resampled, not cropped: the paint API blits a buffer
        // whose dimensions are the rectangle's, so handing it a 1x1 buffer for a 2x2 slot painted
        // nothing at all.
        let scaled = tint.compose_image(&image, (2, 2)).expect("a resampled copy");
        assert_eq!((scaled.width(), scaled.height()), (2, 2));
        assert_eq!(scaled.pixels().len(), 2 * 2 * 4);

        tint.set(Some(Color::rgba(1, 2, 3, 255)));
        let composed = tint.compose_image(&image, (1, 1)).expect("a tinted copy");
        assert_eq!((composed.width(), composed.height()), (1, 1));
        assert_eq!(composed.pixels(), &[1, 2, 3, 200]);
        let again = tint.compose_image(&image, (1, 1)).expect("still tinted");
        assert!(
            composed.shares_allocation_with(&again),
            "the same image, tint and box must reuse the composed copy"
        );

        // The same image going into a different box must be resampled again.
        let resized = tint.compose_image(&image, (3, 3)).expect("resampled");
        assert_eq!((resized.width(), resized.height()), (3, 3));
        assert!(!resized.shares_allocation_with(&composed));

        // An empty image or an empty box has nothing to compose rather than producing a
        // zero-sized handle.
        let empty = crate::image::Image::new();
        assert!(tint.compose_image(&empty, (4, 4)).is_none());
        assert!(tint.compose_image(&image, (0, 4)).is_none());
    }
}
