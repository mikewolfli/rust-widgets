// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Variable faces: the `fvar` axes of a face, and one instance of it at a chosen weight.
//!
//! # What was missing
//!
//! The text layer could select a face by family and name, and the weight machinery
//! ([`crate::core::Font::weight`], [`super::atlas::GlyphKey::weight`]) already existed, but it had
//! nothing to act on: every shipped face carries a single static weight. A host whose UI font is a
//! *variable* face — one face with a `wght` axis, which is what macOS installs for its system UI —
//! could register the bytes but could only ever draw the face's default instance. Asking for
//! `Font::weight() == 700` selected the same bytes and produced the same outlines as 400.
//!
//! This module closes that gap. It answers the three questions the rest of the text layer needs:
//!
//! 1. *Which axes does this face have?* — [`VariableFace::axes`], reading `fvar`.
//! 2. *What coordinate does a weight map to?* — [`VariableFace::clamp_weight`], clamping
//!    `100..=900` to the axis's own range.
//! 3. *What does the glyph look like at that coordinate?* — [`VariableFace::instantiate_at_weight`],
//!    a parsed face with the `wght` coordinate applied.
//!
//! # Why the deltas are not re-implemented here
//!
//! A variable face's glyph variation lives in `gvar` (and a face may also carry `avar` remapping and
//! `HVAR` advance variation). Those are tables with subtle interpolation rules, and the parser this
//! crate already uses for outlines and advances implements them: setting a coordinate on a parsed
//! face makes `outline_glyph`, `glyph_hor_advance` and the shaper's positioning read the varied
//! values. Re-deriving deltas would be a second implementation of the same thing and a second
//! chance for the drawn shape and the measured advance to disagree — the exact defect the text
//! layer was consolidated to remove.
//!
//! # Why a missing axis is not an error
//!
//! Most faces are static, and some variable faces vary something other than weight (`wdth`, `opsz`,
//! a custom axis). For both, the honest answer is the face's **own default instance**: there is no
//! weight axis to move, so moving one would be a fiction. [`VariableFace::axes`] reports an empty
//! list for a static face, [`VariableFace::weight_axis`] is `None` when a variable face has no
//! `wght` axis, and every instantiation path then returns the unmodified face. Nothing panics and
//! nothing is fabricated.
//!
//! # Why the instance is cached per thread
//!
//! An instantiated face is a parse plus a coordinate array, and outlining is a per-glyph, per-frame
//! operation. Parsing once per glyph would put the table-directory walk back on the hot path that
//! [`super::font_assets::with_parsed_face`] exists to keep it off. The cache is thread-local for the
//! same reason the parse caches are: a face is not `Sync` while its coordinates are mutable, and the
//! paint path must not take a lock per glyph. [`MAX_INSTANCES`] bounds it.
//!
//! # Feature gating
//!
//! The whole module is compiled only with `text-shaping`, which is the feature that brings the font
//! parser in. A build without it compiles none of this, and every call site that would use it is
//! gated the same way — so the default build's pixels are unchanged.

#![cfg(feature = "text-shaping")]

use crate::compat::{RefCell, String, Vec};

use super::font_assets::FaceBytes;

/// The `wght` axis tag, the one a [`crate::core::Font::weight`] maps onto.
pub const WEIGHT_AXIS_TAG: [u8; 4] = *b"wght";

const WEIGHT_TAG: ttf_parser::Tag = ttf_parser::Tag::from_bytes(b"wght");

/// How many instantiated faces one thread keeps parsed.
///
/// A UI uses a handful of weights (a regular body, a bold title, perhaps a medium label), and every
/// entry is a parse of a face this build already holds in memory. Eight covers those with room for
/// a caller that also varies another axis, and bounds the cache without an eviction policy that
/// would need bookkeeping on the glyph path.
pub const MAX_INSTANCES: usize = 8;

/// One `fvar` axis of a face.
///
/// The tag is the four-byte OpenType axis identifier (`wght`, `wdth`, `opsz`, or a registered
/// custom tag). Values are in the face's own design units, not normalized coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableAxis {
    /// The four-byte axis tag, exactly as the face declares it.
    pub tag: [u8; 4],
    /// The axis's display name from the `name` table, when the face has a Unicode record for it.
    pub name: Option<String>,
    /// The smallest coordinate the axis accepts.
    pub min: f32,
    /// The coordinate a caller gets when it does not move the axis.
    pub default: f32,
    /// The largest coordinate the axis accepts.
    pub max: f32,
    /// Whether the face marks the axis as hidden from a user-facing axis list.
    pub hidden: bool,
}

impl VariableAxis {
    /// Whether this is the weight axis.
    pub fn is_weight(&self) -> bool {
        self.tag == WEIGHT_AXIS_TAG
    }

    /// The tag as printable text, for a diagnostic.
    ///
    /// A non-ASCII tag byte is replaced with `?`; the value is for display only, never for lookup.
    pub fn tag_text(&self) -> String {
        self.tag
            .iter()
            .map(|byte| if byte.is_ascii_graphic() { *byte as char } else { '?' })
            .collect()
    }
}

/// A face that may have `fvar` axes.
///
/// Cheap to construct (it holds the same two words [`FaceBytes`] does) and cheap to clone; the
/// parse happens inside the methods, through the text layer's existing parse cache.
#[derive(Clone, Copy)]
pub struct VariableFace {
    bytes: FaceBytes,
}

impl VariableFace {
    /// A variable-face view over `face`.
    pub const fn new(face: FaceBytes) -> Self {
        Self { bytes: face }
    }

    /// The face this view reads.
    pub const fn face(self) -> FaceBytes {
        self.bytes
    }

    /// The variable-face view of `family`, when this build carries or a host registered that family.
    ///
    /// The lookup is `super::shaping::face_for_family`, the same one measurement and outlining use,
    /// so a family that resolves here is the face those paths would draw.
    pub fn for_family(family: &str) -> Option<Self> {
        super::shaping::face_for_family(family).map(Self::new)
    }

    /// Whether the face declares an `fvar` table.
    ///
    /// `false` also covers bytes that are not a readable face; a caller that needs to tell those
    /// apart can inspect [`Self::axes`], which is empty for both.
    pub fn is_variable(self) -> bool {
        Self::with_parsed(self.bytes, |face| face.is_variable()).unwrap_or(false)
    }

    /// Every axis the face declares, in the order the face lists them.
    ///
    /// Empty for a static face and for bytes the parser cannot read.
    pub fn axes(self) -> Vec<VariableAxis> {
        Self::with_parsed(self.bytes, |face| {
            face.variation_axes().into_iter().map(|axis| axis_info(face, axis)).collect()
        })
        .unwrap_or_default()
    }

    /// The `wght` axis, when the face has one.
    pub fn weight_axis(self) -> Option<VariableAxis> {
        Self::with_parsed(self.bytes, |face| {
            find_weight_axis(face).map(|axis| axis_info(face, axis))
        })
        .flatten()
    }

    /// The coordinate `weight` maps to on this face's `wght` axis.
    ///
    /// The value is clamped to the axis's declared range, so a face that only spans `300..=700`
    /// answers `700` for a request of `900` rather than pretending it can go further. When the face
    /// has no weight axis the requested value is returned unchanged, and [`Self::instantiate_at_weight`]
    /// then hands back the face's default instance — see the module docs for why that is the honest
    /// answer rather than an error.
    pub fn clamp_weight(self, weight: u16) -> f32 {
        Self::with_parsed(self.bytes, |face| match find_weight_axis(face) {
            Some(axis) => clamp_to_axis(&axis, f32::from(weight)),
            None => f32::from(weight),
        })
        .unwrap_or(f32::from(weight))
    }

    /// A parsed face with `weight` applied to its `wght` axis.
    ///
    /// Returns `None` when the bytes are not a readable face. A static face, or a variable face
    /// without a `wght` axis, yields its default instance rather than an error; the returned
    /// instance reports that through [`VariableInstance::effective_weight`] and
    /// [`VariableInstance::weight_axis`].
    pub fn instantiate_at_weight(self, weight: u16) -> Option<VariableInstance> {
        VariableInstance::with_weight(self.bytes, weight)
    }

    /// A parsed face with an explicit coordinate set applied.
    ///
    /// Each value is clamped to its axis's range by the parser; tags the face does not declare are
    /// ignored. This is the general form for a host that varies an axis other than weight; the
    /// common weight case is [`Self::instantiate_at_weight`], which also reports the effective
    /// weight.
    ///
    /// Returns `None` when the bytes are not a readable face.
    pub fn instantiate_at(self, coordinates: &[([u8; 4], f32)]) -> Option<VariableInstance> {
        VariableInstance::with_coordinates(self.bytes, coordinates)
    }

    /// Runs `f` on the cached parse of `self`.
    fn with_parsed<R>(face: FaceBytes, f: impl FnOnce(&ttf_parser::Face<'_>) -> R) -> Option<R> {
        super::font_assets::with_parsed_face(face, f)
    }
}

/// A parsed face with one coordinate set applied.
///
/// Obtained from [`VariableFace::instantiate_at_weight`] or [`VariableFace::instantiate_at`]. The
/// methods a caller needs for measuring are exposed directly; the outlining method takes a builder
/// implementing [`VariableOutlineBuilder`], which is re-exported so a caller does not have to add
/// the font parser as its own dependency.
pub struct VariableInstance {
    /// The parse with the coordinates applied.
    face: ttf_parser::Face<'static>,
    /// The weight the caller asked for.
    requested_weight: u16,
    /// The `wght` coordinate actually applied (or the axis default when the caller did not move it).
    effective_weight: f32,
    /// The weight axis this instance moved, if the face has one.
    weight_axis: Option<VariableAxis>,
}

impl VariableInstance {
    /// Parses `face` and applies `weight` to its `wght` axis.
    fn with_weight(face: FaceBytes, weight: u16) -> Option<Self> {
        let mut parsed = ttf_parser::Face::parse(face.bytes, 0).ok()?;
        let weight_axis = find_weight_axis(&parsed).map(|axis| axis_info(&parsed, axis));
        let effective_weight = match &weight_axis {
            Some(axis) => clamp_to_values(axis, f32::from(weight)),
            None => f32::from(weight),
        };
        if weight_axis.is_some() {
            // A face with a `wght` axis accepts the coordinate; the parser clamps it again, so the
            // value set here and the value reported above cannot diverge.
            let _ = parsed.set_variation(WEIGHT_TAG, effective_weight);
        }
        Some(Self {
            face: parsed,
            requested_weight: crate::core::Font::normalize_weight(weight),
            effective_weight,
            weight_axis,
        })
    }

    /// Parses `face` and applies an explicit coordinate set.
    fn with_coordinates(face: FaceBytes, coordinates: &[([u8; 4], f32)]) -> Option<Self> {
        let mut parsed = ttf_parser::Face::parse(face.bytes, 0).ok()?;
        for (tag, value) in coordinates {
            let _ = parsed.set_variation(ttf_parser::Tag::from_bytes(tag), *value);
        }
        let weight_axis = find_weight_axis(&parsed).map(|axis| axis_info(&parsed, axis));
        let requested = coordinates
            .iter()
            .find(|(tag, _)| *tag == WEIGHT_AXIS_TAG)
            .map(|(_, value)| *value)
            .unwrap_or_else(|| {
                weight_axis
                    .as_ref()
                    .map(|axis| axis.default)
                    .unwrap_or_else(|| f32::from(crate::core::Font::default_weight()))
            });
        let effective_weight = match &weight_axis {
            Some(axis) => clamp_to_values(axis, requested),
            None => requested,
        };
        Some(Self {
            face: parsed,
            requested_weight: crate::core::Font::normalize_weight(
                requested.round().clamp(0.0, u16::MAX as f32) as u16,
            ),
            effective_weight,
            weight_axis,
        })
    }

    /// The weight the caller asked for, normalized to the `100..=900` scale.
    ///
    /// For [`VariableFace::instantiate_at`] with no `wght` coordinate this reports the round number
    /// closest to the instance's effective coordinate, which is a diagnostic convenience rather than
    /// an input the face saw.
    pub fn requested_weight(&self) -> u16 {
        self.requested_weight
    }

    /// The `wght` coordinate applied to the instance, in the face's design units.
    ///
    /// When the face has no weight axis this is the requested value, and the instance is the face's
    /// default; [`Self::weight_axis`] is the observable that distinguishes the two cases.
    pub fn effective_weight(&self) -> f32 {
        self.effective_weight
    }

    /// The weight axis this instance moved, if the face has one.
    pub fn weight_axis(&self) -> Option<&VariableAxis> {
        self.weight_axis.as_ref()
    }

    /// The face's units per em, the divisor every outline and advance is scaled by.
    pub fn units_per_em(&self) -> u16 {
        self.face.units_per_em()
    }

    /// The glyph that draws `ch`, if the face covers it.
    pub fn glyph_index(&self, ch: char) -> Option<u16> {
        self.face.glyph_index(ch).map(|glyph| glyph.0)
    }

    /// The horizontal advance of `glyph` in the instance's design units.
    ///
    /// This is the varied advance (`hmtx` with `HVAR`/phantom-point deltas applied), which is what
    /// the shaper reports and therefore what a measured line reserves.
    pub fn advance(&self, glyph: u16) -> Option<u16> {
        self.face.glyph_hor_advance(ttf_parser::GlyphId(glyph))
    }

    /// Emits `glyph`'s outline into `builder`, returning whether the face had one.
    ///
    /// The path operators are in design units with y up from the baseline, the same convention the
    /// rasteriser and the outline helpers consume. `false` means the glyph has no outline (a space,
    /// or an id outside the face's glyph count), not that the instance failed.
    pub fn outline_glyph<B>(&self, glyph: u16, builder: &mut B) -> bool
    where
        B: ttf_parser::OutlineBuilder,
    {
        self.face.outline_glyph(ttf_parser::GlyphId(glyph), builder).is_some()
    }
}

/// Re-exported so a caller can implement [`VariableInstance::outline_glyph`]'s builder without
/// naming the font-parser dependency directly.
pub use ttf_parser::OutlineBuilder as VariableOutlineBuilder;

/// The `wght` axis of `face`, when it has one.
fn find_weight_axis(face: &ttf_parser::Face<'_>) -> Option<ttf_parser::VariationAxis> {
    if !face.is_variable() {
        return None;
    }
    face.variation_axes().into_iter().find(|axis| axis.tag == WEIGHT_TAG)
}

/// Converts a parser axis into this module's owned description.
fn axis_info(face: &ttf_parser::Face<'_>, axis: ttf_parser::VariationAxis) -> VariableAxis {
    VariableAxis {
        tag: axis.tag.to_bytes(),
        name: axis_name(face, axis.name_id),
        min: axis.min_value,
        default: axis.def_value,
        max: axis.max_value,
        hidden: axis.hidden,
    }
}

/// Reads the axis display name from the face's `name` table, if it has a Unicode record.
///
/// Decoded here rather than through the parser's convenience string accessor because that accessor
/// is behind the parser's `std` feature, and this module must compile in every profile that enables
/// shaping.
fn axis_name(face: &ttf_parser::Face<'_>, name_id: u16) -> Option<String> {
    let record =
        face.names().into_iter().find(|name| name.name_id == name_id && name.is_unicode())?;
    let mut text = String::new();
    let units = record.name.as_chunks::<2>().0.iter().map(|pair| u16::from_be_bytes(*pair));
    for scalar in core::char::decode_utf16(units) {
        text.push(scalar.ok()?);
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Clamps `value` to `axis`'s range.
fn clamp_to_axis(axis: &ttf_parser::VariationAxis, value: f32) -> f32 {
    value.max(axis.min_value).min(axis.max_value)
}

/// Clamps `value` to an owned axis description's range.
fn clamp_to_values(axis: &VariableAxis, value: f32) -> f32 {
    value.max(axis.min).min(axis.max)
}

// ── Cached instantiation for the per-glyph paths ──

/// One instantiated parse kept on this thread.
struct WeightedTtfFace {
    /// The address that identifies the face's bytes; see [`super::raster`]'s coverage cache.
    bytes: *const u8,
    /// The requested weight, which is the cache key beside the bytes.
    weight: u16,
    /// The parse with the coordinate applied.
    face: ttf_parser::Face<'static>,
}

/// One instantiated shaper parse kept on this thread.
struct WeightedShaperFace {
    /// The address that identifies the face's bytes.
    bytes: *const u8,
    /// The requested weight, which is the cache key beside the bytes.
    weight: u16,
    /// The shaper's parse with the coordinate applied.
    face: rustybuzz::Face<'static>,
}

thread_local! {
    /// Instantiated outline faces, oldest first.
    static WEIGHTED_TTF_FACES: RefCell<Vec<WeightedTtfFace>> = const { RefCell::new(Vec::new()) };
    /// Instantiated shaper faces, oldest first.
    static WEIGHTED_SHAPER_FACES: RefCell<Vec<WeightedShaperFace>> =
        const { RefCell::new(Vec::new()) };
}

/// Whether `face` declares a `wght` axis, answered through the parse cache.
///
/// This is the gate that keeps a static face off the instantiation path: with no axis to move, the
/// existing cached parse is already the correct instance, and a second copy would only cost memory.
fn has_weight_axis(face: FaceBytes) -> bool {
    super::font_assets::with_parsed_face(face, |parsed| find_weight_axis(parsed).is_some())
        .unwrap_or(false)
}

/// The coordinate `weight` maps to on the cached parse of `face`.
fn effective_weight_of(face: &ttf_parser::Face<'_>, weight: u16) -> f32 {
    match find_weight_axis(face) {
        Some(axis) => clamp_to_axis(&axis, f32::from(weight)),
        None => f32::from(weight),
    }
}

/// Runs `f` on an outline parse of `face` instantiated at `weight`.
///
/// Falls through to [`super::font_assets::with_parsed_face`] when the face has no weight axis, so a
/// static face (every face this crate ships) is byte-for-byte on the path it was on before.
pub(crate) fn with_weighted_face<R>(
    face: FaceBytes,
    weight: u16,
    f: impl FnOnce(&ttf_parser::Face<'_>) -> R,
) -> Option<R> {
    if !has_weight_axis(face) {
        return super::font_assets::with_parsed_face(face, f);
    }
    let bytes = face.bytes.as_ptr();
    let entry: Option<WeightedTtfFace> = WEIGHTED_TTF_FACES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) =
            cache.iter().position(|slot| slot.bytes == bytes && slot.weight == weight)
        {
            return Some(cache.remove(index));
        }
        VariableInstance::with_weight(face, weight).map(|instance| WeightedTtfFace {
            bytes,
            weight,
            face: instance.face,
        })
    });

    let answer = entry.as_ref().map(|slot| f(&slot.face));

    if let Some(slot) = entry {
        WEIGHTED_TTF_FACES.with(|cache| {
            let mut cache = cache.borrow_mut();
            // A re-entrant call may already have restored an entry for this key; one copy is enough.
            if !cache.iter().any(|existing| existing.bytes == bytes && existing.weight == weight) {
                cache.push(slot);
                if cache.len() > MAX_INSTANCES {
                    cache.remove(0);
                }
            }
        });
    }
    answer
}

/// Runs `f` on a **shaper** parse of `face` instantiated at `weight`.
///
/// The shaper and the outline path must move an axis the same way or a line would be positioned
/// with one instance and drawn with another. Both therefore go through the same clamping rule
/// ([`effective_weight_of`]) and the same parser call; only the parse cache differs, because the
/// shaper builds layout tables the outline path does not.
pub(crate) fn with_shaper_face_at_weight<R>(
    face: FaceBytes,
    weight: u16,
    f: impl FnOnce(&rustybuzz::Face<'_>) -> R,
) -> Option<R> {
    if !has_weight_axis(face) {
        return super::font_assets::with_shaper_face(face, f);
    }
    let bytes = face.bytes.as_ptr();
    let entry: Option<WeightedShaperFace> = WEIGHTED_SHAPER_FACES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) =
            cache.iter().position(|slot| slot.bytes == bytes && slot.weight == weight)
        {
            return Some(cache.remove(index));
        }
        let mut parsed = rustybuzz::Face::from_slice(face.bytes, 0)?;
        let effective = effective_weight_of(&parsed, weight);
        let _ = parsed.set_variation(WEIGHT_TAG, effective);
        Some(WeightedShaperFace { bytes, weight, face: parsed })
    });

    let answer = entry.as_ref().map(|slot| f(&slot.face));

    if let Some(slot) = entry {
        WEIGHTED_SHAPER_FACES.with(|cache| {
            let mut cache = cache.borrow_mut();
            if !cache.iter().any(|existing| existing.bytes == bytes && existing.weight == weight) {
                cache.push(slot);
                if cache.len() > MAX_INSTANCES {
                    cache.remove(0);
                }
            }
        });
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The weight fixture: one `A` outline and a `wght` axis spanning `100..=900`, with `gvar`
    /// deltas and an `HVAR` advance so the outline *and* the advance move together. See
    /// [`super::super::font_assets::variable_fixtures`] for its provenance and exact geometry.
    const WEIGHT_FIXTURE: &[u8] = super::super::font_assets::variable_fixtures::VARIABLE_WEIGHT_TTF;

    /// A variable face with a `wdth` axis and no `wght` axis: the missing-axis case.
    const WIDTH_FIXTURE: &[u8] = super::super::font_assets::variable_fixtures::VARIABLE_WIDTH_TTF;

    /// The weight fixture plus an `avar` map (normalized `0.2` → `0.6`).
    const AVAR_FIXTURE: &[u8] = super::super::font_assets::variable_fixtures::VARIABLE_AVAR_TTF;

    fn weight_face() -> FaceBytes {
        FaceBytes { name: "Variable Test", bytes: WEIGHT_FIXTURE }
    }

    fn width_face() -> FaceBytes {
        FaceBytes { name: "Variable Width Test", bytes: WIDTH_FIXTURE }
    }

    fn avar_face() -> FaceBytes {
        FaceBytes { name: "Variable Avar Test", bytes: AVAR_FIXTURE }
    }

    /// Records the path an outline builder is fed, so a test can compare two instances by geometry.
    #[derive(Default)]
    struct Recorder {
        points: Vec<(f32, f32)>,
    }

    impl ttf_parser::OutlineBuilder for Recorder {
        fn move_to(&mut self, x: f32, y: f32) {
            self.points.push((x, y));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.points.push((x, y));
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            self.points.push((x1, y1));
            self.points.push((x, y));
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.points.push((x1, y1));
            self.points.push((x2, y2));
            self.points.push((x, y));
        }
        fn close(&mut self) {}
    }

    /// The outline of `A` at `weight`, as design-unit points.
    fn outline_for(face: FaceBytes, weight: u16) -> Vec<(f32, f32)> {
        let instance = VariableFace::new(face)
            .instantiate_at_weight(weight)
            .expect("the fixture is a readable face");
        let glyph = instance.glyph_index('A').expect("the fixture maps A");
        let mut recorder = Recorder::default();
        assert!(instance.outline_glyph(glyph, &mut recorder), "A has an outline");
        recorder.points
    }

    /// The weight fixture's outline of `A` at `weight`.
    fn outline_at(weight: u16) -> Vec<(f32, f32)> {
        outline_for(weight_face(), weight)
    }

    fn bounds(points: &[(f32, f32)]) -> (f32, f32, f32, f32) {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &(x, y) in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        (min_x, min_y, max_x, max_y)
    }

    #[test]
    fn the_fixture_reports_its_axes_with_names_and_ranges() {
        let face = VariableFace::new(weight_face());
        assert!(face.is_variable(), "the fixture declares fvar");
        let axes = face.axes();
        assert_eq!(axes.len(), 1, "the weight fixture has exactly one axis");
        let axis = &axes[0];
        assert_eq!(axis.tag, WEIGHT_AXIS_TAG);
        assert!(axis.is_weight());
        assert_eq!(axis.tag_text(), "wght");
        assert_eq!((axis.min, axis.default, axis.max), (100.0, 400.0, 900.0));
        assert_eq!(axis.name.as_deref(), Some("Weight"), "the fvar axis name is read");
        assert!(!axis.hidden);

        assert_eq!(face.weight_axis().map(|axis| axis.tag), Some(WEIGHT_AXIS_TAG));
    }

    #[test]
    fn a_static_face_reports_no_axes_rather_than_guessing() {
        // The compiled faces are static. Whichever this build ships, none declares fvar.
        for face in super::super::font_assets::active_faces() {
            let variable = VariableFace::new(*face);
            assert!(!variable.is_variable(), "{} is not a variable face", face.name);
            assert!(variable.axes().is_empty(), "{} must not invent an axis", face.name);
            assert!(variable.weight_axis().is_none(), "{} has no weight axis", face.name);
        }
    }

    #[test]
    fn weight_700_draws_a_different_outline_than_400() {
        let regular = outline_at(400);
        let bold = outline_at(700);
        assert!(!regular.is_empty() && !bold.is_empty());
        assert_ne!(regular, bold, "the gvar deltas must move the outline");

        let (rx0, ry0, rx1, ry1) = bounds(&regular);
        let (bx0, by0, bx1, by1) = bounds(&bold);
        assert_eq!((rx0, ry0), (bx0, by0), "the fixture keeps the left and bottom edges fixed");
        assert!(bx1 > rx1, "the 700 master widens the right edge: {bx1} > {rx1}");
        assert!(by1 > ry1, "the 700 master raises the top edge: {by1} > {ry1}");
    }

    #[test]
    fn the_instance_advance_moves_with_the_weight() {
        let face = VariableFace::new(weight_face());
        let regular = face.instantiate_at_weight(400).expect("readable");
        let bold = face.instantiate_at_weight(700).expect("readable");
        let glyph = regular.glyph_index('A').expect("A is mapped");
        let regular_advance = regular.advance(glyph).expect("hmtx has an advance");
        let bold_advance = bold.advance(glyph).expect("hmtx has an advance");
        assert!(
            bold_advance > regular_advance,
            "the 700 master's wider advance must reach the instance: {bold_advance} > {regular_advance}"
        );
        assert_eq!(regular.effective_weight(), 400.0);
        assert_eq!(bold.effective_weight(), 700.0);
    }

    #[test]
    fn a_weight_beyond_the_axis_is_clamped_to_the_axis_bounds() {
        let face = VariableFace::new(weight_face());
        assert_eq!(face.clamp_weight(1000), 900.0);
        assert_eq!(face.clamp_weight(0), 100.0);
        assert_eq!(face.clamp_weight(450), 450.0, "an in-range value is untouched");

        let low = face.instantiate_at_weight(0).expect("readable");
        let at_min = face.instantiate_at_weight(100).expect("readable");
        assert_eq!(low.effective_weight(), 100.0);
        assert_eq!(at_min.effective_weight(), 100.0);
        assert_eq!(low.requested_weight(), 100, "a requested 0 normalizes to the scale's floor");
        assert_eq!(outline_at(0), outline_at(100), "below-range requests land on the minimum");

        let high = face.instantiate_at_weight(1000).expect("readable");
        let at_max = face.instantiate_at_weight(900).expect("readable");
        assert_eq!(high.effective_weight(), 900.0);
        assert_eq!(at_max.effective_weight(), 900.0);
        assert_eq!(outline_at(1000), outline_at(900), "above-range requests land on the maximum");
    }

    /// `avar` remaps the normalized coordinate before the parser applies the deltas.
    ///
    /// The fixture's segment map turns normalized `0.2` into `0.6`, which is the `gvar` peak, so a
    /// user request of 500 reaches exactly the instance 700 does. The linear face is the control:
    /// its 500 is a fifth of the way to the moved master, so the two faces must not agree there.
    #[test]
    fn avar_remaps_the_weight_coordinate_before_the_deltas_are_applied() {
        let face = VariableFace::new(avar_face());
        assert!(face.is_variable());
        let axis = face.weight_axis().expect("avar does not change the fvar axes");
        assert_eq!((axis.min, axis.default, axis.max), (100.0, 400.0, 900.0));

        let at_400 = outline_for(avar_face(), 400);
        let at_500 = outline_for(avar_face(), 500);
        let at_700 = outline_for(avar_face(), 700);
        // The `avar` map stores its coordinates as F2Dot14, so the remapped point is
        // 0.5999755859375 rather than exactly 0.6 and the two requests can differ by a fraction
        // of a font unit. Compare within a unit: the claim is that they reach one instance, not
        // that the binary fixed-point rounding is bit-identical.
        let near = |a: &[(f32, f32)], b: &[(f32, f32)]| {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(p, q)| (p.0 - q.0).abs() <= 1.0 && (p.1 - q.1).abs() <= 1.0)
        };
        assert!(
            near(&at_500, &at_700),
            "avar maps user 500 onto the normalized coordinate that 700 reaches: \
             {at_500:?} vs {at_700:?}"
        );
        assert_ne!(at_500, at_400, "the remapped instance still differs from the default");

        // The control: on the face without avar, 500 is between 400 and 700.
        let linear_500 = outline_for(weight_face(), 500);
        assert_ne!(linear_500, outline_at(700), "linear normalization does not saturate at 500");
        assert_ne!(at_500, linear_500, "the avar table must change the drawn shape");

        // The advance is varied from the same remapped coordinate, so measurement agrees.
        let instance_500 = face.instantiate_at_weight(500).expect("readable");
        let instance_700 = face.instantiate_at_weight(700).expect("readable");
        let instance_400 = face.instantiate_at_weight(400).expect("readable");
        let glyph = instance_500.glyph_index('A').expect("A is mapped");
        let advance_500 = instance_500.advance(glyph).expect("hmtx has an advance");
        let advance_700 = instance_700.advance(glyph).expect("hmtx has an advance");
        assert!(
            advance_500.abs_diff(advance_700) <= 1,
            "the remapped advance is the 700 instance's: {advance_500} vs {advance_700}"
        );
        assert_ne!(advance_500, instance_400.advance(glyph).expect("hmtx has an advance"));
        assert_ne!(
            instance_500.advance(glyph),
            face.instantiate_at_weight(400).expect("readable").advance(glyph)
        );
    }

    #[test]
    fn a_variable_face_without_a_weight_axis_uses_its_default_instance() {
        let face = VariableFace::new(width_face());
        assert!(face.is_variable(), "the width fixture is a variable face");
        assert!(face.weight_axis().is_none(), "it varies width, not weight");
        assert_eq!(face.axes().len(), 1);
        assert_eq!(face.axes()[0].tag, *b"wdth");
        assert_eq!(face.clamp_weight(700), 700.0, "no axis means nothing to clamp against");

        let regular = face.instantiate_at_weight(400).expect("readable");
        let bold = face.instantiate_at_weight(700).expect("readable");
        assert!(regular.weight_axis().is_none());
        assert_eq!(regular.effective_weight(), 400.0);
        assert_eq!(bold.effective_weight(), 700.0);
        assert_eq!(regular.requested_weight(), 400);
        assert_eq!(bold.requested_weight(), 700);

        let glyph = regular.glyph_index('A').expect("A is mapped");
        assert_eq!(
            regular.advance(glyph),
            bold.advance(glyph),
            "without a weight axis the advance must not move"
        );

        let mut regular_outline = Recorder::default();
        let mut bold_outline = Recorder::default();
        assert!(regular.outline_glyph(glyph, &mut regular_outline));
        assert!(bold.outline_glyph(glyph, &mut bold_outline));
        assert_eq!(
            regular_outline.points, bold_outline.points,
            "without a weight axis both weights are the face's default instance"
        );
    }

    #[test]
    fn unreadable_bytes_answer_none_instead_of_panicking() {
        let junk = FaceBytes { name: "junk", bytes: b"not a font" };
        let face = VariableFace::new(junk);
        assert!(!face.is_variable());
        assert!(face.axes().is_empty());
        assert!(face.weight_axis().is_none());
        assert!(face.instantiate_at_weight(700).is_none());
        assert!(face.instantiate_at(&[(WEIGHT_AXIS_TAG, 700.0)]).is_none());

        // The cached per-glyph path answers the same way rather than panicking.
        assert!(with_weighted_face(junk, 700, |_| ()).is_none());
        assert!(with_shaper_face_at_weight(junk, 700, |_| ()).is_none());
    }

    #[test]
    fn an_explicit_coordinate_set_instantiates_the_same_outline_as_a_weight() {
        let face = VariableFace::new(weight_face());
        let by_weight = face.instantiate_at_weight(700).expect("readable");
        let by_coordinates = face.instantiate_at(&[(WEIGHT_AXIS_TAG, 700.0)]).expect("readable");
        assert_eq!(by_coordinates.effective_weight(), 700.0);
        assert_eq!(by_weight.effective_weight(), by_coordinates.effective_weight());

        let glyph = by_weight.glyph_index('A').expect("A is mapped");
        let mut weight_outline = Recorder::default();
        let mut coordinate_outline = Recorder::default();
        assert!(by_weight.outline_glyph(glyph, &mut weight_outline));
        assert!(by_coordinates.outline_glyph(glyph, &mut coordinate_outline));
        assert_eq!(weight_outline.points, coordinate_outline.points);
        assert_eq!(by_weight.advance(glyph), by_coordinates.advance(glyph));

        // An unknown tag is ignored, and the weight axis keeps its default.
        let untouched = face.instantiate_at(&[(*b"zzzz", 999.0)]).expect("readable");
        assert_eq!(untouched.effective_weight(), 400.0);
    }

    #[test]
    fn the_cached_instance_agrees_with_a_fresh_one() {
        let face = weight_face();
        let fresh = VariableFace::new(face).instantiate_at_weight(700).expect("readable");
        let mut fresh_outline = Recorder::default();
        let glyph = fresh.glyph_index('A').expect("A is mapped");
        assert!(fresh.outline_glyph(glyph, &mut fresh_outline));

        let mut cached_outline = Recorder::default();
        let answer = with_weighted_face(face, 700, |parsed| {
            parsed.outline_glyph(ttf_parser::GlyphId(glyph), &mut cached_outline).is_some()
        });
        assert_eq!(answer, Some(true));
        assert_eq!(
            cached_outline.points, fresh_outline.points,
            "the cached per-glyph instance must draw the same outline as the public one"
        );

        // A second call is served from the cache and still agrees.
        let mut again = Recorder::default();
        assert_eq!(
            with_weighted_face(face, 700, |parsed| {
                parsed.outline_glyph(ttf_parser::GlyphId(glyph), &mut again).is_some()
            }),
            Some(true)
        );
        assert_eq!(again.points, fresh_outline.points);
    }

    /// The shaper's weighted form measures the face's varied advances, and its unweighted form is
    /// the face's own instance.
    #[test]
    fn the_shaper_measures_a_variable_face_at_the_requested_weight() {
        let shaper = super::super::shaping::RustybuzzShaper::new(weight_face())
            .expect("the fixture is readable");
        let regular = shaper.shape_face_at_weight("AA", 48.0, 1.0, 400).expect("shaped");
        let bold = shaper.shape_face_at_weight("AA", 48.0, 1.0, 700).expect("shaped");
        assert!(
            bold.width > regular.width,
            "700 must measure wider than 400: {} > {}",
            bold.width,
            regular.width
        );
        assert_eq!(bold.glyph_ids, regular.glyph_ids, "the same characters are mapped");

        // This fixture's default instance is its 400 master, so omitting the weight agrees with
        // asking for 400 — that is what the unweighted render path relies on.
        let own = shaper.shape_face("AA", 48.0, 1.0).expect("shaped");
        assert_eq!(own.width, regular.width);
    }

    /// The measuring path (the shaper) and the drawing path (the outline cache) must move the axis
    /// identically: same clamp, same instance, same advance. This is the test the issue asks for,
    /// expressed at the text layer's own boundary.
    #[test]
    fn the_shaper_instance_and_the_outline_instance_move_the_axis_together() {
        let face = weight_face();
        let reference = VariableFace::new(face).instantiate_at_weight(1000).expect("readable");
        assert_eq!(reference.effective_weight(), 900.0, "an out-of-range request clamps first");
        let glyph = reference.glyph_index('A').expect("A is mapped");

        let mut outline_outline = Recorder::default();
        assert!(reference.outline_glyph(glyph, &mut outline_outline));

        let mut shaper_outline = Recorder::default();
        let shaper = with_shaper_face_at_weight(face, 1000, |parsed| {
            let _ = parsed.outline_glyph(ttf_parser::GlyphId(glyph), &mut shaper_outline);
            parsed.glyph_hor_advance(ttf_parser::GlyphId(glyph))
        })
        .flatten();

        assert_eq!(
            shaper_outline.points, outline_outline.points,
            "the shaper's instance and the outline instance must be the same face"
        );
        assert_eq!(
            shaper,
            reference.advance(glyph),
            "the measured advance must come from the same instance as the drawn outline"
        );
    }

    /// The host's own variable face, when this host has one, declares a weight axis and moves it.
    ///
    /// This is the real-face counterpart of the synthetic fixture: it proves the API reads a face
    /// the crate does not ship. The candidate comes from [`crate::platform::Platform::system_fonts`],
    /// so this test carries no OS knowledge of its own — no `cfg(target_os)` and no system font path
    /// (principle #36). A host that reports no variable face able to express both 400 and 700 has
    /// nothing to test here, which is why that case is a pass rather than a failure.
    #[test]
    fn the_host_system_ui_face_declares_a_weight_axis() {
        // Probe with the parser first: the only face this test leaks is one that really can draw
        // `A` at both weights and really moves the axis between them, so a host whose first
        // variable family cannot outline the glyph is skipped rather than failed.
        let found = crate::platform::get_platform()
            .system_fonts()
            .into_iter()
            .filter_map(|font| font.source)
            .find_map(|path| {
                let bytes = std::fs::read(path).ok()?;
                if bytes.is_empty() {
                    return None;
                }
                let mut parsed = ttf_parser::Face::parse(&bytes, 0).ok()?;
                let axis = super::find_weight_axis(&parsed)?;
                if axis.min_value > 400.0 || axis.max_value < 700.0 {
                    return None;
                }
                let glyph = parsed.glyph_index('A')?;
                let mut regular = Recorder::default();
                parsed.set_variation(super::WEIGHT_TAG, 400.0)?;
                parsed.outline_glyph(glyph, &mut regular)?;
                let mut bold = Recorder::default();
                parsed.set_variation(super::WEIGHT_TAG, 700.0)?;
                parsed.outline_glyph(glyph, &mut bold)?;
                if regular.points == bold.points {
                    return None;
                }
                Some(bytes)
            });
        let Some(bytes) = found else {
            return;
        };
        let bytes: &'static [u8] = crate::compat::Box::leak(bytes.into_boxed_slice());
        let face = VariableFace::new(FaceBytes { name: "System UI", bytes });
        assert!(face.is_variable(), "a host face the parser reports as variable is one");
        let axis = face.weight_axis().expect("it declares a wght axis");
        assert!(axis.min < axis.max, "the weight axis has a range");

        let regular = face.instantiate_at_weight(400).expect("readable");
        let bold = face.instantiate_at_weight(700).expect("readable");
        let glyph = regular.glyph_index('A').expect("the face covers A");
        let mut regular_outline = Recorder::default();
        let mut bold_outline = Recorder::default();
        assert!(regular.outline_glyph(glyph, &mut regular_outline));
        assert!(bold.outline_glyph(glyph, &mut bold_outline));
        assert_ne!(
            regular_outline.points, bold_outline.points,
            "the host face must draw a different outline at the two weights"
        );
    }

    /// A static face must take the pre-existing cached path at any requested weight, which is what
    /// keeps the default build's geometry and pixels byte-identical.
    #[test]
    fn a_static_face_is_unchanged_at_every_weight() {
        for face in super::super::font_assets::active_faces() {
            let glyph = super::super::font_assets::with_parsed_face(*face, |parsed| {
                parsed.glyph_index('A').or_else(|| parsed.glyph_index('中'))
            })
            .flatten();
            let Some(glyph) = glyph else { continue };

            let mut regular = Recorder::default();
            let mut bold = Recorder::default();
            assert_eq!(
                with_weighted_face(*face, 400, |parsed| {
                    parsed.outline_glyph(glyph, &mut regular).is_some()
                }),
                Some(true)
            );
            assert_eq!(
                with_weighted_face(*face, 700, |parsed| {
                    parsed.outline_glyph(glyph, &mut bold).is_some()
                }),
                Some(true)
            );
            assert_eq!(
                regular.points, bold.points,
                "{} has no weight axis, so both requests are its one instance",
                face.name
            );
            assert_eq!(
                super::super::font_assets::with_parsed_face(*face, |parsed| parsed
                    .glyph_hor_advance(glyph)),
                with_weighted_face(*face, 700, |parsed| parsed.glyph_hor_advance(glyph)),
                "{} has no weight axis, so its advance must not move with the weight",
                face.name
            );
        }
    }
}
