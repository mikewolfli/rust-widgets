// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Lightweight, already-resolved icon content for widget presentation.
//!
//! # What a widget needs beyond "one icon source"
//!
//! Three things, and this module owns all three so a control does not invent them:
//!
//! * [`IconSource`] — a drawable glyph, either a crate token or static geometry.
//! * [`SharedIconPixels`] — validated RGBA8 bytes plus the fit rule for an icon slot.
//! * `IconModeSet` — the **state variants** of one logical icon: its on/off pair, its
//!   idle/hover/pressed renditions and its disabled rendition. A control that changes
//!   glyph with its state (a check box's tick, a lock's open/closed pair, a speaker's
//!   mute) declares that set once, and the fallback rule is applied here rather than in
//!   each draw path.
//!
//! High-density (`@2x`) sources are not a separate type: the host registers a second
//! source under the same name at density 2 (see
//! [`crate::widget::icon_registry::register_icon_at_density`]), and the icon widget selects
//! it when its scale factor calls for it. That keeps the *name* the identity and the
//! density an attribute of the stored source.

use crate::compat::String;
#[cfg(all(feature = "image", not(alloc_frugal)))]
use crate::compat::Vec;
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

    /// A copy of these pixels recoloured as a **monochrome mask**.
    ///
    /// The alpha channel is kept (so an icon's anti-aliased edges survive) and scaled by the
    /// tint's own alpha; the RGB channels are replaced by the tint's. That is what
    /// [`IconColorMode::Monochrome`] declares a tintable resource to be, and it is the only
    /// reading under which "tint a picture" is well defined — a photographic source keeps its own
    /// colours and should not be tinted at all.
    ///
    /// The result is an ordinary [`SharedIconPixels`], so a caller can cache it and share it like
    /// the source. The invariant `pixels.len() == width * height * 4` holds by construction, which
    /// is why this does not route through the validating constructor and cannot fail.
    pub fn tinted(&self, tint: crate::core::Color) -> Self {
        // The handle's own constructor refused a zero extent and a mismatched length, so the
        // borrowed form cannot fail here; the expectation names that invariant instead of silently
        // falling back to an untinted copy.
        Self::tinted_rgba8(self.pixels.as_slice(), self.width, self.height, tint)
            .expect("a validated shared icon always satisfies the tint invariants")
    }

    /// [`Self::tinted`] from **borrowed** RGBA8 bytes.
    ///
    /// Exists for a source that is not a handle yet — a control's legacy `Image` keeps its bytes in
    /// its own allocation — so tinting it does not need a throw-away handle first. The tint rule
    /// and its arithmetic live in one place ([`Self::tinted`] delegates here), so the two entry
    /// points cannot disagree.
    ///
    /// `None` when the extent is zero or `pixels` is not exactly `width * height * 4` bytes. The
    /// refusal is reported rather than padded: a buffer of the wrong length describes an extent
    /// this handle would then report wrongly.
    pub fn tinted_rgba8(
        pixels: &[u8],
        width: u32,
        height: u32,
        tint: crate::core::Color,
    ) -> Option<Self> {
        let expected = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
        if expected == 0 || pixels.len() != expected {
            return None;
        }
        let mut out = Vec::with_capacity(expected);
        for pixel in pixels.as_chunks::<4>().0 {
            let alpha = ((pixel[3] as u16 * tint.a as u16) / 255) as u8;
            out.extend_from_slice(&[tint.r, tint.g, tint.b, alpha]);
        }
        Some(Self { pixels: Arc::new(out), width, height })
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

/// A control state that can supply its **own glyph**, rather than a tint of one glyph.
///
/// # Why this is distinct from the paint state
///
/// The icon widget's paint state (normal / disabled / selected) changes how one glyph is
/// *painted*. This enum answers a different question: a check box's tick, a lock's
/// open/closed pair and a speaker's mute are **different shapes**, and no tint derived
/// from one outline can produce the other. A control declares the set through
/// [`IconModeSet`] and asks it for the shape its current state calls for.
///
/// The names are the states a control already has ([`crate::style::WidgetState`] for the
/// interactive ones), reduced to the ones that commonly change a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconMode {
    /// The idle appearance.
    Normal,
    /// The pointer is over the control.
    Hovered,
    /// The control is held.
    Pressed,
    /// The control's boolean value is on (a tick, a closed lock).
    Checked,
    /// The control's boolean value is off (an empty box, an open lock).
    Unchecked,
    /// The control is disabled.
    Disabled,
}

/// The glyph variants of one logical icon, with an explicit fallback per state.
///
/// # Fallback rule
///
/// A state that declares no source falls back along the chain a user would expect, never
/// to silence:
///
/// | Asked | Falls back to |
/// |---|---|
/// | `Hovered` | `Normal` |
/// | `Pressed` | `Hovered`, then `Normal` |
/// | `Checked` | `Normal` |
/// | `Unchecked` | `Normal` |
/// | `Disabled` | `Normal` |
///
/// This is what makes the set safe to declare partially: a control that only has an on/off
/// pair still paints while hovered, and a control that only has an idle glyph still paints
/// when disabled. Falling back to `None` instead would make a missing variant look like a
/// missing icon.
///
/// # Why `Default` is empty
///
/// An empty set answers `None` for every state, which is "this control declares no state
/// variants" — the behaviour of every control before this type existed. A control opts in
/// by constructing a set, so the default cannot change an existing rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IconModeSet {
    normal: Option<IconSource>,
    hovered: Option<IconSource>,
    pressed: Option<IconSource>,
    checked: Option<IconSource>,
    unchecked: Option<IconSource>,
    disabled: Option<IconSource>,
}

impl IconModeSet {
    /// A set with no variants at all; every lookup answers `None`.
    pub const EMPTY: Self = Self {
        normal: None,
        hovered: None,
        pressed: None,
        checked: None,
        unchecked: None,
        disabled: None,
    };

    /// Starts a set from the glyph every state falls back to.
    pub const fn new(normal: IconSource) -> Self {
        Self {
            normal: Some(normal),
            hovered: None,
            pressed: None,
            checked: None,
            unchecked: None,
            disabled: None,
        }
    }

    /// Declares the glyph for `mode`, replacing any earlier declaration.
    ///
    /// A builder rather than a setter so a control can state the whole group where it
    /// builds its icon:
    ///
    /// ```text
    /// let icons = IconModeSet::new(idle)
    ///     .with(IconMode::Checked, tick)
    ///     .with(IconMode::Unchecked, empty_box);
    /// ```
    pub const fn with(mut self, mode: IconMode, source: IconSource) -> Self {
        match mode {
            IconMode::Normal => self.normal = Some(source),
            IconMode::Hovered => self.hovered = Some(source),
            IconMode::Pressed => self.pressed = Some(source),
            IconMode::Checked => self.checked = Some(source),
            IconMode::Unchecked => self.unchecked = Some(source),
            IconMode::Disabled => self.disabled = Some(source),
        }
        self
    }

    /// The glyph declared for exactly `mode`, without any fallback.
    ///
    /// A caller uses this to tell "the control declared this state" from "this state
    /// resolved to the idle glyph": the two paint the same shape but mean different
    /// things to a test or a state inspector.
    pub const fn declared(&self, mode: IconMode) -> Option<IconSource> {
        match mode {
            IconMode::Normal => self.normal,
            IconMode::Hovered => self.hovered,
            IconMode::Pressed => self.pressed,
            IconMode::Checked => self.checked,
            IconMode::Unchecked => self.unchecked,
            IconMode::Disabled => self.disabled,
        }
    }

    /// The glyph to paint in `mode`, applying the documented fallback chain.
    ///
    /// `None` only when the set has no `Normal` source and no source for the requested
    /// state — the empty set, or a set whose only declarations are for states this one
    /// does not fall back to.
    pub const fn resolve(&self, mode: IconMode) -> Option<IconSource> {
        match mode {
            IconMode::Normal => self.normal,
            IconMode::Hovered => match self.hovered {
                Some(source) => Some(source),
                None => self.normal,
            },
            IconMode::Pressed => match self.pressed {
                Some(source) => Some(source),
                None => match self.hovered {
                    Some(source) => Some(source),
                    None => self.normal,
                },
            },
            IconMode::Checked => match self.checked {
                Some(source) => Some(source),
                None => self.normal,
            },
            IconMode::Unchecked => match self.unchecked {
                Some(source) => Some(source),
                None => self.normal,
            },
            IconMode::Disabled => match self.disabled {
                Some(source) => Some(source),
                None => self.normal,
            },
        }
    }

    /// Whether the set declares no glyph at all.
    pub const fn is_empty(&self) -> bool {
        self.normal.is_none()
            && self.hovered.is_none()
            && self.pressed.is_none()
            && self.checked.is_none()
            && self.unchecked.is_none()
            && self.disabled.is_none()
    }
}

/// Tracks one icon-resource binding's asynchronous lifecycle, so a late or failed result cannot
/// replace newer content.
///
/// # Why this type exists at all, given that the crate has no async loader
///
/// §19.7 puts the asynchronous lifecycle in the **host/loading adapter**, not in the widget: a
/// validated [`IconSource`] is committed to the control synchronously, and the widget API needs no
/// `Pending` state. What the widget layer cannot do on the host's behalf is the bookkeeping that
/// makes late results safe, and that bookkeeping has four rules which are easy to get subtly wrong
/// and are exactly what §19.13 cases I10 and I11 ask about:
///
/// | Situation | Required behaviour |
/// |---|---|
/// | a newer request started, then an older one finishes | the older result is **not** applied |
/// | an update fails while content is live | the last-valid content stays, and the failure is reported |
/// | the caller clears the icon | the content is really removed, and any in-flight request is invalidated |
/// | a result arrives for a revision older than the one on screen | it is **not** applied |
///
/// A host may of course implement this itself; this type is the crate's *executable statement* of the
/// rules, so a host that uses it inherits the tested behaviour rather than re-deriving it. It holds no
/// lock, no I/O and no task — it is a version tag and three fields, which is why it costs nothing in a
/// widget that does not use resources.
///
/// # Ownership: how "do not touch a destroyed consumer" is achieved
///
/// The binding is owned by whatever owns the widget (a view-model, a loader entry, the host's own
/// map), and [`Self::complete`] takes `&mut self`. A result that arrives after that owner is gone
/// therefore has no binding to call: the host's late callback holds whatever weak handle it was given
/// and finds nothing to upgrade. The type deliberately does not offer a detached "apply this
/// somewhere" entry point, because that is precisely the API that makes touching a destroyed consumer
/// possible.
#[derive(Debug, Default)]
pub struct IconResourceBinding {
    /// The id handed to the newest [`Self::begin`], and the revision it was for.
    in_flight: Option<(u64, u64)>,
    /// Monotonic request counter; never reused, so a stale id can never equal a current one.
    next_request: u64,
    /// The **last valid** content and the revision it belongs to.
    ready: Option<(u64, IconSource)>,
}

/// What a completed load turned into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconLoadOutcome {
    /// The result is the current one and is now in effect.
    Applied(IconSource),
    /// The load failed. `kept` is the last-valid content still in effect (`None` when there was none,
    /// which is the first-load failure §19.7 requires to be reported as a real failure).
    Failed {
        /// The content still in effect after the failure.
        kept: Option<IconSource>,
        /// The host's diagnostic for why the load failed.
        diagnostic: crate::compat::String,
    },
    /// The result was not applied: a newer request had started, the binding had been cleared, or the
    /// loaded revision is older than the content already on screen.
    Stale,
}

impl IconResourceBinding {
    /// A binding with no content.
    pub const fn new() -> Self {
        Self { in_flight: None, next_request: 0, ready: None }
    }

    /// Starts a load for `revision` and returns the request id to present with its result.
    ///
    /// Calling this again **invalidates** the previous request, which is what makes "B started, A
    /// finished last" resolve in B's favour (I10).
    pub fn begin(&mut self, revision: u64) -> u64 {
        self.next_request += 1;
        self.in_flight = Some((self.next_request, revision));
        self.next_request
    }

    /// Presents the result of a load started by [`Self::begin`].
    pub fn complete(
        &mut self,
        request_id: u64,
        revision: u64,
        result: Result<IconSource, crate::compat::String>,
    ) -> IconLoadOutcome {
        // Only the newest request may change anything. A result for a superseded request id, or one
        // that arrives after `clear`, is dropped here rather than applied and then corrected.
        if self.in_flight != Some((request_id, revision)) {
            return IconLoadOutcome::Stale;
        }
        self.in_flight = None;
        match result {
            Ok(source) => {
                // A *successful* result for an older revision must not replace newer content either:
                // the resource moved backwards, and the newer render is the one the user saw.
                if let Some((ready_revision, _)) = self.ready {
                    if revision < ready_revision {
                        return IconLoadOutcome::Stale;
                    }
                }
                self.ready = Some((revision, source));
                IconLoadOutcome::Applied(source)
            }
            Err(diagnostic) => {
                // Last-valid stays: a failed update must not blank an icon that is working (I11).
                let kept = self.ready.as_ref().map(|(_, source)| *source);
                IconLoadOutcome::Failed { kept, diagnostic }
            }
        }
    }

    /// The content currently in effect, if any.
    pub fn current(&self) -> Option<IconSource> {
        self.ready.as_ref().map(|(_, source)| *source)
    }

    /// The revision the current content belongs to.
    pub fn revision(&self) -> Option<u64> {
        self.ready.as_ref().map(|(revision, _)| *revision)
    }

    /// Whether a load is in flight.
    pub fn is_loading(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Removes the content and invalidates any in-flight request.
    ///
    /// This is the "user explicitly clears the icon" rule: the binding goes back to empty and a
    /// result that was already on its way can no longer bring the old icon back.
    pub fn clear(&mut self) {
        self.in_flight = None;
        self.ready = None;
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
        let image =
            Image::try_from_rgba(bytes, 2, 3).expect("the buffer matches its declared size");
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

#[cfg(test)]
mod binding_tests {
    use super::{IconLoadOutcome, IconResourceBinding, IconSource};
    use crate::widget::IconName;

    fn source(name: IconName) -> IconSource {
        IconSource::Builtin(name)
    }

    /// blue27 §19.13 case I10: with two requests in flight and the **older** one finishing last, only
    /// the current request may be applied.
    ///
    /// This is the ordering a real loader produces all the time — a slow first read overtaken by a
    /// fast second one — and applying A there would leave the control showing a resource the binding
    /// no longer names, with no further event to correct it.
    #[test]
    fn a_superseded_result_is_not_applied_when_it_finishes_last() {
        let mut binding = IconResourceBinding::new();

        let request_a = binding.begin(1);
        let request_b = binding.begin(2);
        assert!(binding.is_loading());

        // B finishes first and wins; A arrives afterwards and must change nothing.
        assert_eq!(
            binding.complete(request_b, 2, Ok(source(IconName::Save))),
            IconLoadOutcome::Applied(source(IconName::Save))
        );
        assert_eq!(
            binding.complete(request_a, 1, Ok(source(IconName::Search))),
            IconLoadOutcome::Stale,
            "a superseded request must be dropped, not applied and then corrected"
        );
        assert_eq!(binding.current(), Some(source(IconName::Save)));
        assert_eq!(binding.revision(), Some(2));
        assert!(!binding.is_loading(), "the completed request is no longer in flight");
    }

    /// A successful result for an **older** revision than the one on screen must not roll it back.
    #[test]
    fn an_older_revision_does_not_replace_newer_content() {
        let mut binding = IconResourceBinding::new();
        let first = binding.begin(7);
        assert_eq!(
            binding.complete(first, 7, Ok(source(IconName::Save))),
            IconLoadOutcome::Applied(source(IconName::Save))
        );

        let stale = binding.begin(6);
        assert_eq!(
            binding.complete(stale, 6, Ok(source(IconName::Search))),
            IconLoadOutcome::Stale,
            "the resource went backwards; the newer render is what the user already saw"
        );
        assert_eq!(binding.revision(), Some(7));
    }

    /// blue27 §19.13 case I11: a failed update keeps the last-valid content and reports the failure;
    /// an explicit clear afterwards really removes it, and an in-flight result cannot revive it.
    #[test]
    fn a_failed_update_keeps_the_last_valid_content_until_an_explicit_clear() {
        let mut binding = IconResourceBinding::new();

        // First load fails: there is no last-valid to keep, and the failure is reported as such
        // rather than as an empty success.
        let first = binding.begin(1);
        assert_eq!(
            binding.complete(first, 1, Err(crate::compat::String::from("missing file"))),
            IconLoadOutcome::Failed {
                kept: None,
                diagnostic: crate::compat::String::from("missing file"),
            }
        );
        assert_eq!(binding.current(), None);

        // A good load lands, then an update fails: the working icon stays.
        let good = binding.begin(2);
        assert_eq!(
            binding.complete(good, 2, Ok(source(IconName::Save))),
            IconLoadOutcome::Applied(source(IconName::Save))
        );
        let failed_update = binding.begin(3);
        assert_eq!(
            binding.complete(failed_update, 3, Err(crate::compat::String::from("io error"))),
            IconLoadOutcome::Failed {
                kept: Some(source(IconName::Save)),
                diagnostic: crate::compat::String::from("io error"),
            }
        );
        assert_eq!(binding.current(), Some(source(IconName::Save)), "last-valid must survive");

        // The user clears it: the content goes, and the result still in flight cannot bring it back.
        let in_flight = binding.begin(4);
        binding.clear();
        assert_eq!(binding.current(), None, "an explicit clear must really remove the icon");
        assert!(!binding.is_loading(), "clearing invalidates the in-flight request");
        assert_eq!(
            binding.complete(in_flight, 4, Ok(source(IconName::Search))),
            IconLoadOutcome::Stale,
            "a result that was already on its way must not undo the clear"
        );
        assert_eq!(binding.current(), None);
    }

    /// A default-constructed binding is empty, so a control that never saw a resource is not
    /// distinguishable from one whose resource was cleared — which is the point.
    #[test]
    fn a_fresh_binding_holds_nothing() {
        let binding = IconResourceBinding::new();
        assert_eq!(binding.current(), None);
        assert_eq!(binding.revision(), None);
        assert!(!binding.is_loading());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::IconName;

    fn source(name: IconName) -> IconSource {
        IconSource::Builtin(name)
    }

    /// A tinted copy recolours the mask and keeps the alpha, which is what "tint" means here.
    ///
    /// The source is 2x1: a half-transparent white pixel and an opaque one. The result must carry
    /// the tint's RGB, scale each alpha by the tint's own alpha, and leave the dimensions alone —
    /// the draw fits the icon slot from them.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn tinting_replaces_rgb_and_keeps_a_scaled_alpha() {
        let pixels =
            SharedIconPixels::from_rgba8(vec![255, 255, 255, 128, 255, 255, 255, 255], 2, 1)
                .expect("valid RGBA8 data");

        let tinted = pixels.tinted(crate::core::Color::rgba(10, 20, 30, 255));
        assert_eq!((tinted.width(), tinted.height()), (2, 1));
        assert_eq!(tinted.pixels(), &[10, 20, 30, 128, 10, 20, 30, 255]);

        let half = pixels.tinted(crate::core::Color::rgba(10, 20, 30, 128));
        assert_eq!(
            half.pixels()[3],
            64,
            "a half-transparent tint must halve the source alpha rather than ignore it"
        );
        assert!(
            !tinted.shares_allocation_with(&pixels),
            "a tint must produce its own pixels, not alias the source"
        );
    }

    /// The fallback chain is the whole contract: a partially declared group still paints.
    ///
    /// A control with only an on/off pair must survive hover, press and disabled without
    /// the icon vanishing — silence is the failure mode this rule exists to prevent.
    #[test]
    fn a_partial_mode_set_resolves_through_the_documented_fallback() {
        let idle = source(IconName::Check);
        let ticked = source(IconName::Star);
        let group = IconModeSet::new(idle).with(IconMode::Checked, ticked);

        assert_eq!(group.resolve(IconMode::Normal), Some(idle));
        assert_eq!(group.resolve(IconMode::Checked), Some(ticked));
        assert_eq!(group.resolve(IconMode::Unchecked), Some(idle), "no unchecked glyph");
        assert_eq!(group.resolve(IconMode::Hovered), Some(idle), "hover falls back to idle");
        assert_eq!(group.resolve(IconMode::Disabled), Some(idle), "disabled falls back to idle");
        assert_eq!(group.declared(IconMode::Checked), Some(ticked));
        assert_eq!(group.declared(IconMode::Unchecked), None, "not declared, only resolved");
        assert!(!group.is_empty());
    }

    /// `Pressed` prefers a pressed glyph, then the hover glyph, then the idle one.
    #[test]
    fn pressed_prefers_its_own_glyph_then_hover_then_normal() {
        let idle = source(IconName::Check);
        let hover = source(IconName::Star);
        let press = source(IconName::Cross);

        let full =
            IconModeSet::new(idle).with(IconMode::Hovered, hover).with(IconMode::Pressed, press);
        assert_eq!(full.resolve(IconMode::Pressed), Some(press));

        let without_press = IconModeSet::new(idle).with(IconMode::Hovered, hover);
        assert_eq!(without_press.resolve(IconMode::Pressed), Some(hover));

        let bare = IconModeSet::new(idle);
        assert_eq!(bare.resolve(IconMode::Pressed), Some(idle));
    }

    /// The empty set declares nothing, which is how a control opts out.
    ///
    /// This is the default every control has until it constructs a set, so it has to
    /// answer `None` rather than inventing a glyph.
    #[test]
    fn the_default_mode_set_declares_no_glyph() {
        let empty = IconModeSet::default();
        assert!(empty.is_empty());
        for mode in [
            IconMode::Normal,
            IconMode::Hovered,
            IconMode::Pressed,
            IconMode::Checked,
            IconMode::Unchecked,
            IconMode::Disabled,
        ] {
            assert_eq!(empty.resolve(mode), None, "{mode:?} must not invent a source");
            assert_eq!(empty.declared(mode), None);
        }
        assert_eq!(IconModeSet::EMPTY, empty);
    }
}
