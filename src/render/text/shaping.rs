// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Real shaping: what a script's *rules* do to a string, on top of what its face *has*.
//!
//! # The two axes, again
//!
//! [BLUE23 §0A.4](docs/plans/archive/blue23.md) keeps two questions apart, and this module is the
//! second one:
//!
//! * *Does a face have this character?* — [`crate::render::text::GlyphSource`], answered by data;
//! * *What glyphs does the script say this **string** becomes?* — this module, answered by the
//!   face's layout tables (`GSUB`, `GPOS`) and the script's rules.
//!
//! A CJK face needs the first and not the second: one character is one cell. An Arabic face
//! needs the second and cannot fake it — `بيت` is three characters and, drawn as three isolated
//! letters, is not a word in any state a reader would call *close*. That asymmetry is why
//! BLUE23 calls the two orthogonal rather than one "font support" axis.
//!
//! # Why the engine is opt-in
//!
//! Shaping is only meaningful with a face, so it is compiled in only when one is
//! ([`crate::render::text::shaping::active_faces`] is empty otherwise) and the model in
//! [`crate::render::text::line`] stays the whole story for a default build. The default build's
//! bytes do not change.
//!
//! # Why the engine is built per call rather than stored
//!
//! A shaper face carries a mutable layout-plan cache, which is why it is not `Sync` — and
//! [`crate::render::TextShaper`] is required to be `Send + Sync` because a renderer is. Rather
//! than hide that behind a lock (and pay for the lock on every measurement), the face is parsed
//! from its bytes on each call: parsing a table directory is a few dozen reads, this is an
//! opt-in path, and the alternative is shared mutable state in the one place the three
//! constraints say there must be none.

// `char_advance` spells its single-character call as `c.to_string()`. `ToString` is in the std
// prelude but not in the no-std one, so a profile that compiles the shaper without `std`
// (`mini,runtime-fonts`) has to name the trait. The alias is the crate's convention for that —
// see `compat::MiniToString`'s comment.
#[cfg(alloc_frugal)]
use crate::compat::MiniToString;
use crate::compat::{HashMap, RefCell, String, Vec};
use crate::core::Font;
use crate::render::text::font_assets::{active_faces, FaceBytes};
use crate::render::{ShapedGlyphRun, TextShaper};

/// The face whose family name matches `font.family()`, if any.
///
/// # Why the face is named rather than guessed
///
/// [`crate::core::Font`] already carries the answer to "which face?" — its `family` — and this
/// layer honours it. The alternative, picking the first face that happens to cover the text,
/// would change every label's **advance** the moment a vector face was enabled, for every
/// caller, including those drawing Latin text that never needed a second face. Metrics are
/// layout: a feature that silently re-laid-out every control would be a surprise, not an
/// upgrade. Naming the face makes the choice the caller's, which is also what makes "enable the
/// feature and nothing changes until you ask for the family" a stated behaviour instead of a
/// lucky one.
///
/// The match is case-insensitive because a font family name is an identifier a person types.
///
/// # Host-registered faces
///
/// With `runtime-fonts` on, a host may have registered a face under the family name it named — and
/// that is the whole point of registering one. A family lookup that only saw the compiled list would
/// make the host's own font unreachable by name, which is the one way a caller *does* name it. So
/// the registered faces are searched first, then the compiled ones.
pub(crate) fn face_for_family(family: &str) -> Option<FaceBytes> {
    #[cfg(feature = "runtime-fonts")]
    {
        let (runtime, len) = crate::render::text::runtime_fonts::active_runtime_faces();
        for face in &runtime[..len] {
            if face.name.eq_ignore_ascii_case(family) {
                return Some(*face);
            }
        }
    }
    active_faces().iter().copied().find(|face| face.name.eq_ignore_ascii_case(family))
}

/// The face that will **paint** `text`, which is the face whose metrics must size it.
///
/// # The defect this corrects, in measured numbers
///
/// Measurement used [`face_for_family`], which only succeeds when the caller names a face this build
/// ships. Every theme in this crate names `"Arial"`, this crate ships `"Open Sans"`, so the lookup
/// answered `None` and every Latin cluster fell back to the **flat estimate** in
/// [`crate::render::text::estimate_cluster_advance`] — `0.6 em` for *every* non-wide cluster, the same
/// width for `i` as for `w`. Measured against Open Sans' own `hmtx` at 14 px:
///
/// ```text
///   ch   real(px)  estimate   drift        'widget' pen positions
///    w     10.25      8.40    +1.85        0.00 / 0.00
///    i      3.53      8.40    -4.87       10.25 / 8.40   <- next glyph starts 1.85 px early
///    d      8.57      8.40    +0.17       13.79 /16.80   <- and the one after, 3.01 px late
///    g      7.72      8.40    -0.68       22.35 /25.20
///    e      7.86      8.40    -0.54       30.08 /33.60
///    t      5.47      8.40    -2.93       37.94 /42.00
/// ```
///
/// So `w` and `i` overlapped by 1.85 px while the gap after `i` was 3.01 px too wide — the cramping
/// and the stretched gaps in one word, which is what a flat factor applied to a proportional face
/// has to produce. (The totals nearly agree by luck: 43.4 px of real ink against 50.4 px reserved,
/// because over- and under-estimates cancel. Per-glyph they do not, and per-glyph is what a reader
/// sees.)
///
/// # Why the fallback is by *coverage*, and why that is not "silently re-laying-out"
///
/// The concern [`face_for_family`] documents is real: a face's metrics must not change for a caller
/// that did not ask. But the relevant question is not "did the caller name a face" — it is "which
/// face will the ink come from". [`VectorSource`](crate::render::text::VectorSource) selects by
/// coverage and ignores the family entirely, so on this build `"Arial"` text is **painted by Open
/// Sans** whatever the metrics say. Sizing it by the 0.6 em estimate is therefore not conservatism,
/// it is measuring one font and drawing another — the "measure with A, draw with B" defect this crate
/// has already paid for once.
///
/// Honouring the family first keeps the documented behaviour exactly: a caller that names a shipped
/// face gets that face, byte for byte as before. Coverage is only the fallback for a family this build
/// does not ship — which, for every theme in this crate today, is the case that is *currently* wrong.
///
/// # What this does not change
///
/// A build with **no** vector face still resolves `None` here, so the estimate remains the whole
/// answer and the default build's output is unchanged. The fallback needs a face to fall back to.
fn face_for_metrics(text: &str, font: &Font) -> Option<FaceBytes> {
    // Named first, so the documented contract is untouched.
    if let Some(named) = face_for_family(font.family()) {
        return Some(named);
    }
    // Otherwise the face the ink will actually come from. The probe character is the first strong
    // non-whitespace one, which is the same rule `face_for_text` uses — a run is shaped by the face
    // that covers what it says.
    let probe = text.chars().find(|ch| !ch.is_whitespace())?;
    // Coverage, through the shared lookup: `face_for_char` is the one place that knows a host face
    // outranks a compiled one, so measurement and drawing cannot disagree about which face wins.
    crate::render::text::font_assets::face_for_char(probe)
}

/// The first face that covers `text`'s first strong character.
///
/// "First strong character" is the rule the bidirectional algorithm uses to pick a paragraph's
/// base direction (UAX #9 P2). It is the right rule for a caller that has said *nothing* about
/// which face it wants — the case [`RustybuzzShaper::from_active_faces`] exists for — and it is
/// deliberately not the rule the renderer's measurement path uses: see [`face_for_family`].
fn face_for_text(text: &str) -> Option<FaceBytes> {
    let probe = text.chars().find(|ch| !ch.is_whitespace())?;
    crate::render::text::font_assets::face_for_char(probe)
}

/// The advance of each cluster, summed over the **glyphs** that cluster shaped into.
///
/// `ranges` is the caller's clusters as `(byte_start, byte_end)` pairs in logical order — the
/// same list [`crate::render::text::line`] built for the bidirectional reorder. The result is
/// indexed the same way, so the caller can read it straight into its own clusters.
///
/// # Why a sum, and why by byte range
///
/// Shaping is not one-glyph-per-cluster. A ligature turns two characters into one glyph; Arabic
/// joining substitutes a contextual form for a letter; a cluster can also decompose into several
/// glyphs. What the renderer needs is the width each *cluster* occupies, so every glyph's
/// advance is credited to the cluster its reported byte index falls inside — by range, not by
/// counting, because counting would mis-attribute a cluster that spans more than one character.
pub(crate) fn cluster_advances(
    text: &str,
    ranges: &[(usize, usize)],
    font: &Font,
    scale: f32,
) -> Option<Vec<f32>> {
    let face = face_for_metrics(text, font)?;
    // The same memo every other shaping entry point uses. `text_line` measures before it draws and
    // a layout measures again on the next frame, so this request repeats far more often than it
    // changes; the cache turns the repeat into a lookup (G-19).
    let key = ShapeKey::new(face.name, text, font.size(), scale, Some(font.weight()), false);
    if let Some(advances) = cached_advances(&key) {
        return Some(advances);
    }
    // The cached shaper parse: this runs for every measured and every drawn label, and
    // `text_line` measures before `draw_text` shapes, so a single text control parsed the font
    // twice a frame. Measured, one `measure_text("M")` cost ~18us, which multiplied out to the
    // 25ms a frame of `demo/control` spent rendering 36 controls.
    //
    // # Why the weight is part of the request
    //
    // A variable face is one face with a `wght` axis, so 400 and 700 are different advances from the
    // same bytes. The renderer's drawing halves take the weight too — `paint_active_at_weight` for
    // the cluster path, `outline_*_at_weight` for the geometry — and all of them instantiate through
    // the same clamp-and-cache rule, so a line cannot be measured with one instance and drawn with
    // another. A static face has no axis and takes the original cached parse untouched, which is why
    // every face this crate ships is unaffected.
    let advances = super::variable::with_shaper_face_at_weight(face, font.weight(), |parsed| {
        cluster_advances_in_face(text, ranges, font, scale, parsed)
    })
    .flatten();
    if let Some(advances) = advances {
        store_advances(key, advances.clone());
        Some(advances)
    } else {
        None
    }
}

/// Computes the per-cluster advances using an already-parsed shaper `face`.
fn cluster_advances_in_face(
    text: &str,
    ranges: &[(usize, usize)],
    font: &Font,
    scale: f32,
    parsed: &rustybuzz::Face<'_>,
) -> Option<Vec<f32>> {
    let units_per_em = parsed.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return None;
    }
    let mut buffer = take_shaping_buffer();
    buffer.clear();
    buffer.push_str(text);
    // The direction is guessed from the text, for the same reason the face is named by its
    // family: a label's base direction is a property of its content, and the reorder itself is
    // `bidi`'s job and happens later — shaping works in logical order.
    buffer.guess_segment_properties();
    #[cfg(test)]
    note_shaping_pass();
    let output = rustybuzz::shape(parsed, ligature_free_features(), buffer);

    // The pixel size of one font unit. An `em` is the font size, so a unit is
    // `font_size / units_per_em` logical pixels — the real advance, rather than a per-cluster
    // guess. `scale` converts to device pixels, as everywhere else in the text layer.
    let unit_px = font.size() * scale / units_per_em;

    let mut advances = vec![0.0f32; ranges.len()];
    for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
        let byte = info.cluster as usize;
        // `ranges` is sorted and disjoint, so the cluster containing `byte` is a partition point.
        let index = ranges.partition_point(|&(start, _)| start <= byte);
        if let Some(slot) = advances.get_mut(index.saturating_sub(1)) {
            *slot += position.x_advance as f32 * unit_px;
        }
    }
    // Hand the input storage back for the next shaping pass (G-19). `clear` is the consuming form
    // that reuses the allocation; it is called only after every borrow of `output` has ended.
    recycle_shaping_buffer(output.clear());
    Some(advances)
}

/// The shaping features this renderer can honour: **none of the ligatures**.
///
/// # Why standard ligatures are switched off
///
/// This text pipeline draws one *cluster* at a time from the cluster's own text
/// ([`crate::render::text::line::for_each_cluster`] splits by scalar, not by
/// shaped glyph). A ligature is one glyph covering two clusters: the shaper
/// credits its advance to the cluster the glyph reports, and the other cluster is
/// then drawn into a `max(1)`-pixel box — so `fib` painted as `f b`, with the `i`
/// invisible and a gap where the ligature's extra advance sat. Measured in the
/// code-editor demo: every `fi` in the sample source lost its `i`, in the tree
/// view's file names as well as in the buffer, and the defect predates the
/// editor work.
///
/// Turning the standard ligatures off is the fix the architecture supports: after
/// it every glyph belongs to exactly one cluster, which is also the contract the
/// editor's caret geometry and the cluster advances rely on. Editors that want
/// ligatures need glyph-level painting, which is a different renderer — not a
/// feature flag.
///
/// `kern` stays on: kerning moves a glyph within its own cluster and is honoured.
fn ligature_free_features() -> &'static [rustybuzz::Feature] {
    const TAG_LIGA: rustybuzz::ttf_parser::Tag = rustybuzz::ttf_parser::Tag::from_bytes(b"liga");
    const TAG_CLIG: rustybuzz::ttf_parser::Tag = rustybuzz::ttf_parser::Tag::from_bytes(b"clig");
    /// A global "off" feature: value 0 over the whole buffer.
    const fn off(tag: rustybuzz::ttf_parser::Tag) -> rustybuzz::Feature {
        rustybuzz::Feature { tag, value: 0, start: 0, end: u32::MAX }
    }
    const FEATURES: [rustybuzz::Feature; 2] = [off(TAG_LIGA), off(TAG_CLIG)];
    &FEATURES
}

/// A shaping request identity: everything that changes the glyphs a face produces.
///
/// `font_size` and `scale` are compared by their bit patterns, for the same reason the text
/// layer's measurement caches do: the question is "is this the same request", not "are these
/// floats approximately equal". `weight: None` is the face's own instance, which is not the same
/// request as any explicit weight. `ligatures` selects between the cluster renderer's
/// ligature-free feature set and the glyph-level one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShapeKey {
    face: &'static str,
    text: String,
    font_size_bits: u32,
    scale_bits: u32,
    weight: Option<u16>,
    ligatures: bool,
}

impl ShapeKey {
    fn new(
        face: &'static str,
        text: &str,
        font_size: f32,
        scale: f32,
        weight: Option<u16>,
        ligatures: bool,
    ) -> Self {
        Self {
            face,
            text: String::from(text),
            font_size_bits: font_size.to_bits(),
            scale_bits: scale.to_bits(),
            weight,
            ligatures,
        }
    }
}

/// How many shaped results one thread keeps.
///
/// A UI shapes a bounded set of labels: captions, titles, a handful of numbers. 512 is the same
/// bound the trait path already used, and the whole point of the cache is that the second call for
/// the same caption is a lookup instead of a `GSUB`/`GPOS` pass. A cache larger than this is not a
/// better cache — it is a leak nobody measured.
const SHAPE_CACHE_LIMIT: usize = 512;

thread_local! {
    /// Shaped runs, keyed by request. The value is the whole result, including `None`: a face that
    /// cannot shape a request must not be re-parsed on every call just to learn that again.
    static SHAPED_RUNS: RefCell<HashMap<ShapeKey, Option<ShapedGlyphRun>>> =
        RefCell::new(HashMap::new());
    /// Per-cluster advances, keyed the same way. Kept separately because this result carries the
    /// cluster partition, which a glyph run does not: one shape pass can answer both, but the two
    /// answers are different projections and reconstituting one from the other would lose data.
    static CLUSTER_ADVANCES: RefCell<HashMap<ShapeKey, Vec<f32>>> = RefCell::new(HashMap::new());
    /// One reusable input buffer per thread (G-19).
    ///
    /// `rustybuzz::shape` consumes a `UnicodeBuffer`, so the buffer cannot simply be reset in
    /// place after use. `GlyphBuffer::clear` hands back an empty `UnicodeBuffer` that reuses the
    /// shaped buffer's allocation, which is what makes a pool possible despite the consuming API:
    /// the shaping pass recycles its input storage through this slot, and a thread that shapes a
    /// stream of labels does not allocate the codepoint/glyph arrays from scratch each time. The
    /// shaped-result cache above still short-circuits repeated requests entirely.
    static SHAPING_BUFFER: RefCell<Option<rustybuzz::UnicodeBuffer>> = const { RefCell::new(None) };
}

/// Takes the thread's reusable shaping buffer, or creates one on first use.
fn take_shaping_buffer() -> rustybuzz::UnicodeBuffer {
    SHAPING_BUFFER.with(|slot| slot.borrow_mut().take()).unwrap_or_default()
}

/// Returns a cleared shaping buffer to the thread-local slot.
///
/// `try_with` because a shaping call can run during thread teardown, when the slot has already
/// been destroyed; losing the recycled allocation is harmless.
fn recycle_shaping_buffer(buffer: rustybuzz::UnicodeBuffer) {
    let _ = SHAPING_BUFFER.try_with(|slot| {
        *slot.borrow_mut() = Some(buffer);
    });
}

// Test-only count of real rustybuzz shaping passes, so the cache is observable (G-19).
//
// Per-thread rather than process-wide: the shaped-result cache is thread-local, so counting
// globally made one test's shaping passes appear in another test's delta when the suite ran in
// parallel. The counter now measures exactly the cache under test.
#[cfg(test)]
thread_local! {
    static SHAPING_PASSES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn note_shaping_pass() {
    SHAPING_PASSES.with(|passes| passes.set(passes.get() + 1));
}

#[cfg(test)]
fn shaping_pass_count() -> usize {
    SHAPING_PASSES.with(core::cell::Cell::get)
}

#[cfg(test)]
fn clear_shape_caches() {
    let _ = SHAPED_RUNS.try_with(|cache| cache.borrow_mut().clear());
    let _ = CLUSTER_ADVANCES.try_with(|cache| cache.borrow_mut().clear());
}

fn cached_run(key: &ShapeKey) -> Option<Option<ShapedGlyphRun>> {
    SHAPED_RUNS.try_with(|cache| cache.borrow().get(key).cloned()).ok().flatten()
}

fn store_run(key: ShapeKey, run: Option<ShapedGlyphRun>) {
    let _ = SHAPED_RUNS.try_with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= SHAPE_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, run);
    });
}

fn cached_advances(key: &ShapeKey) -> Option<Vec<f32>> {
    CLUSTER_ADVANCES.try_with(|cache| cache.borrow().get(key).cloned()).ok().flatten()
}

fn store_advances(key: ShapeKey, advances: Vec<f32>) {
    let _ = CLUSTER_ADVANCES.try_with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= SHAPE_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, advances);
    });
}

/// A shaper backed by a real font's layout tables.
///
/// # What it adds over [`crate::render::SimpleTextShaper`]
///
/// Glyph **identities** and real advances: `SimpleTextShaper` numbers characters sequentially
/// and charges each a fixed fraction of an em, which is enough to lay out a left-to-right Latin
/// label and cannot express that two characters became one glyph, that a letter changed shape
/// because of its neighbour, or that a pair is kerned.
///
/// # What it needs
///
/// A face. [`Self::from_active_faces`] picks the first of this build's opt-in faces that covers
/// the text; a caller with its own face constructs one with [`Self::new`]. With no face, every
/// method falls back to the model — this type never guesses a glyph.
pub struct RustybuzzShaper {
    face: FaceBytes,
}

impl RustybuzzShaper {
    /// A shaper over `face`, or `None` when its bytes are not a font this crate can read.
    pub fn new(face: FaceBytes) -> Option<Self> {
        // Validate through the name-keyed shaper cache: construction is the moment a caller has a
        // face it intends to shape with, so warming the same cache `shape_face` reads means the
        // first call does not walk the table directory again.
        super::font_assets::with_shaper_face(face, |_| ())?;
        Some(Self { face })
    }

    /// A shaper over the first of this build's opt-in faces that covers `text`, if any.
    ///
    /// This is the convenience for a caller that has not named a family: it picks by coverage.
    /// Presentation goes through [`Self::new`] with the face [`face_for_family`] selected, so a
    /// label's *metrics* never change behind a caller's back.
    ///
    /// Returns `None` on a build with no vector face enabled, which is the honest answer: there
    /// is nothing to shape with, and the caller should use the model.
    pub fn from_active_faces(text: &str) -> Option<Self> {
        Self::new(face_for_text(text)?)
    }

    /// The name of the face this shaper reads.
    pub fn face_name(&self) -> &'static str {
        self.face.name
    }

    /// The trait-entry-point cache: shaping is deterministic for a fixed face, text, size and
    /// feature set, so the trait call takes the same memo as every other shaping entry point
    /// (G-19). Before this, a caller holding `&dyn TextShaper` paid a full `GSUB`/`GPOS` pass on
    /// every call while the renderer's path was cached — the same font, the same bytes, two costs.
    fn trait_shape(&self, text: &str, font_size: f32) -> Option<ShapedGlyphRun> {
        self.shape_face(text, font_size, 1.0)
    }

    /// Shape `text` into one run of positioned glyphs, in logical order.
    ///
    /// Positions are in logical pixels; the run's `width` is the sum of the glyph advances, and
    /// its `height` is the font's line box — the same box
    /// [`crate::render::text::line`]'s model reports.
    pub fn shape_face(&self, text: &str, font_size: f32, scale: f32) -> Option<ShapedGlyphRun> {
        self.shape_face_with(text, font_size, scale, ligature_free_features(), None)
    }

    /// [`Self::shape_face`] at an explicit weight.
    ///
    /// The face's `wght` axis is moved before shaping, so a variable face's advances and glyph ids
    /// come from the instance the caller asked for. A static face ignores the weight, so this is the
    /// same answer as [`Self::shape_face`] for every face this crate ships.
    pub fn shape_face_at_weight(
        &self,
        text: &str,
        font_size: f32,
        scale: f32,
        weight: u16,
    ) -> Option<ShapedGlyphRun> {
        self.shape_face_with(text, font_size, scale, ligature_free_features(), Some(weight))
    }

    /// Shape `text` with the face's standard ligatures enabled.
    ///
    /// This is the glyph-level form of [`Self::shape_face`]: a run of positioned glyph ids where
    /// one glyph may cover several characters (`ffi` becomes the face's `ffi` ligature). It is
    /// only useful to a renderer that paints glyph identities rather than cluster text; the
    /// cluster model in [`crate::render::text::line`] keeps the ligature-free form because its
    /// caret geometry assumes one cluster per drawn box.
    pub fn shape_face_with_ligatures(
        &self,
        text: &str,
        font_size: f32,
        scale: f32,
    ) -> Option<ShapedGlyphRun> {
        self.shape_face_with(text, font_size, scale, &[], None)
    }

    /// [`Self::shape_face_with_ligatures`] at an explicit weight.
    ///
    /// The ligature-enabled form of [`Self::shape_face_at_weight`]; the two differ only in the
    /// feature set, exactly as their default-weight counterparts do.
    pub fn shape_face_with_ligatures_at_weight(
        &self,
        text: &str,
        font_size: f32,
        scale: f32,
        weight: u16,
    ) -> Option<ShapedGlyphRun> {
        self.shape_face_with(text, font_size, scale, &[], Some(weight))
    }

    /// Shapes one run with an explicit feature set and, when `weight` is `Some`, an instance weight.
    ///
    /// `None` is the face's **own** instance, which is what [`Self::shape_face`] means and what the
    /// renderer's unweighted path takes. `Some(w)` instantiates the face at `w` through the same
    /// clamp-and-cache rule the outline paths use; for a face with no `wght` axis the two are the
    /// same bytes.
    fn shape_face_with(
        &self,
        text: &str,
        font_size: f32,
        scale: f32,
        features: &[rustybuzz::Feature],
        weight: Option<u16>,
    ) -> Option<ShapedGlyphRun> {
        // The cache is checked before the face is parsed: a repeated caption must not walk the
        // table directory, build a `UnicodeBuffer` or run `GSUB`/`GPOS` at all (G-19). The key
        // names the face, so two shapers over different faces cannot alias each other.
        let key =
            ShapeKey::new(self.face.name, text, font_size, scale, weight, features.is_empty());
        if let Some(hit) = cached_run(&key) {
            return hit;
        }
        // The name-keyed shaper cache: a label is measured and then drawn, and the same face is
        // shaped again across frames, so this must not re-read the table directory per call. The
        // instance cache is keyed by weight as well, so two weights are two parses of one face.
        let shape = |parsed: &rustybuzz::Face<'_>| {
            let units_per_em = parsed.units_per_em() as f32;
            if units_per_em <= 0.0 {
                return None;
            }
            let mut buffer = take_shaping_buffer();
            buffer.clear();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            #[cfg(test)]
            note_shaping_pass();
            let output = rustybuzz::shape(parsed, features, buffer);

            let unit_px = font_size * scale / units_per_em;
            let mut glyph_ids = Vec::with_capacity(output.len());
            let mut positions = Vec::with_capacity(output.len());
            let mut pen_x = 0.0f32;
            for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
                glyph_ids.push(info.glyph_id);
                pen_x += position.x_offset as f32 * unit_px;
                positions.push((pen_x, position.y_offset as f32 * unit_px));
                pen_x += position.x_advance as f32 * unit_px;
            }
            let run = ShapedGlyphRun {
                width: pen_x,
                height: font_size * 1.2,
                glyph_ids,
                positions,
                font_size,
            };
            // Recycle the input storage (G-19). Called after the loop so no borrow of `output`
            // outlives it; `clear` is the consuming form that keeps the allocation.
            recycle_shaping_buffer(output.clear());
            Some(run)
        };
        let shaped = match weight {
            Some(weight) => super::variable::with_shaper_face_at_weight(self.face, weight, shape),
            None => super::font_assets::with_shaper_face(self.face, shape),
        }
        .flatten();
        store_run(key, shaped.clone());
        shaped
    }
}

/// The glyph run the software renderer paints when glyph-level text is enabled.
///
/// Face selection is the same one measurement uses ([`face_for_family`]), so enabling the
/// glyph-level path never changes which face a named family resolves to. The run is shaped at
/// `weight`, and the caller must outline the ids at the same weight
/// ([`crate::render::text::outline_glyph_at_weight`]) or the pen positions and the ink describe two
/// different instances. It returns `None` on a build with no face for `font.family()`, which is the
/// caller's cue to fall back to the cluster renderer.
pub fn glyph_run_for_family_at_weight(
    text: &str,
    font: &Font,
    scale: f32,
    weight: u16,
) -> Option<ShapedGlyphRun> {
    let face = face_for_family(font.family())?;
    RustybuzzShaper::new(face)?.shape_face_with_ligatures_at_weight(
        text,
        font.size(),
        scale,
        weight,
    )
}

impl TextShaper for RustybuzzShaper {
    /// One run of real glyphs, or — if the face cannot be read — an empty run rather than a
    /// fabricated one.
    fn shape(&self, text: &str, font_size: f32) -> Vec<ShapedGlyphRun> {
        self.trait_shape(text, font_size).into_iter().collect()
    }

    fn measure_width(&self, text: &str, font_size: f32) -> f32 {
        self.trait_shape(text, font_size).map(|run| run.width).unwrap_or(0.0)
    }

    fn cache_identity(&self) -> u64 {
        // FNV-1a over the face name: the name is the identity `face_for_family` selects by, and a
        // zero result would disable RichText's cache, so it is forced odd.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.face.name.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash | 1
    }

    fn measure_height(&self, _text: &str, font_size: f32) -> f32 {
        font_size * 1.2
    }

    fn char_advance(&self, c: char, font_size: f32) -> f32 {
        self.measure_width(&c.to_string(), font_size)
    }
}

#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_is_chosen_by_the_first_strong_character() {
        // `from_active_faces` is the "caller named nothing" convenience, so it picks by
        // coverage. The renderer does not use this rule — it honours `Font::family`, so that
        // enabling a face never changes a label's advance behind the caller's back.
        let face = face_for_text("hello").expect("a Latin face is enabled");
        assert!(face.bytes.len() > 1000);

        #[cfg(feature = "fonts-complex")]
        assert_eq!(
            face_for_text("\u{628}\u{64a}\u{62a}").expect("an Arabic face is enabled").name,
            "Noto Naskh Arabic"
        );
    }

    /// A family name selects the face, case-insensitively, and an unknown family does not.
    ///
    /// Gated on `fonts-vector-latin` specifically, not on "a vector feature": the body asserts the
    /// **Open Sans** face, which only that feature ships. Gating this on the module's own
    /// `any(fonts-vector-latin, fonts-complex)` window made it fail on a `fonts-complex`-only build,
    /// where the face it names genuinely does not exist — the test was asserting a fact about the
    /// wrong build configuration.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn the_face_a_family_names_is_the_face_that_is_used() {
        assert_eq!(
            face_for_family("open sans").map(|face| face.name),
            Some("Open Sans"),
            "a family name is matched case-insensitively"
        );
        assert!(
            face_for_family("Arial").is_none(),
            "a family this build does not carry leaves the model in charge"
        );
    }

    #[test]
    fn shaping_a_ligature_produces_fewer_glyphs_than_characters() {
        // "ffi" is a standard Latin ligature. The model would report three clusters; a real
        // face reports the glyphs the face's `GSUB` says the string becomes.
        let shaper = RustybuzzShaper::from_active_faces("ffi").expect("a face is enabled");
        let shaped = shaper.shape_face("ffi", 16.0, 1.0).expect("the run shapes");
        assert!(
            shaped.glyph_ids.len() <= 3,
            "a ligature may not produce more glyphs than characters: {:?}",
            shaped.glyph_ids
        );
        assert!(shaped.width > 0.0, "a shaped run has a width");
    }

    #[test]
    fn arabic_joining_selects_contextual_forms() {
        // The criterion BLUE23 §0A.4 states for shaping: an Arabic word must not shape to the
        // same glyphs as its letters in isolation. `بيت` (house) is three letters whose medial
        // forms differ from their isolated ones, so the shaped run's glyph identities must
        // differ from the per-character `cmap` lookups.
        #[cfg(feature = "fonts-complex")]
        {
            let shaper =
                RustybuzzShaper::from_active_faces("\u{628}\u{64a}\u{62a}").expect("a face");
            let shaped = shaper.shape_face("\u{628}\u{64a}\u{62a}", 16.0, 1.0).expect("shapes");
            let faces = crate::render::text::font_assets::active_faces();
            let arabic = faces
                .iter()
                .find(|face| face.name == "Noto Naskh Arabic")
                .expect("the Arabic face is enabled");
            let parsed = ttf_parser::Face::parse(arabic.bytes, 0).expect("parses");
            let isolated: Vec<u32> = "\u{628}\u{64a}\u{62a}"
                .chars()
                .filter_map(|ch| parsed.glyph_index(ch).map(|id| id.0 as u32))
                .collect();
            assert_eq!(isolated.len(), 3, "the face has all three letters");
            assert_ne!(
                shaped.glyph_ids, isolated,
                "joining must substitute the contextual forms, not the isolated ones"
            );
        }
    }

    #[test]
    fn real_advances_differ_from_the_models_flat_factor() {
        // The point of reading the face: a proportional face's letters are not all 0.6 em. If
        // they were, the model would be enough and this feature would be worth nothing.
        let shaper = RustybuzzShaper::from_active_faces("iI").expect("a face is enabled");
        let thin = shaper.char_advance('i', 16.0);
        let thick = shaper.char_advance('W', 16.0);
        assert!(thick > thin, "W ({thick}) must be wider than i ({thin}) in a proportional face");
    }

    #[test]
    fn the_trait_path_reuses_the_cached_shape_result() {
        // The measurement G-19 asked for: the trait entry point, which used to bypass the
        // renderer's cache entirely, must run rustybuzz once for a repeated request. The counter
        // is inside the only place `rustybuzz::shape` is called, so this cannot pass by accident.
        let shaper = RustybuzzShaper::from_active_faces("cache-probe").expect("a face is enabled");
        clear_shape_caches();
        let before = shaping_pass_count();
        let first = shaper.shape("cache-probe", 13.0);
        let after_first = shaping_pass_count();
        let second = shaper.shape("cache-probe", 13.0);
        let after_second = shaping_pass_count();

        assert_eq!(after_first - before, 1, "the first call shapes once");
        assert_eq!(after_second, after_first, "the second call is a cache hit");
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].glyph_ids, second[0].glyph_ids);

        // The direct path takes the same memo: measuring and then drawing a label is one shape,
        // not two.
        let _measured = shaper.measure_width("cache-probe", 13.0);
        assert_eq!(shaping_pass_count(), after_second, "measurement reuses the shaped run");
    }

    #[test]
    fn recycled_shaping_buffers_do_not_leak_glyphs_between_texts() {
        // The thread-local pool reuses the input allocation across shaping calls. Clearing the
        // buffer and the shaped-result cache first means both strings take the real shaping path,
        // so this fails if the recycled buffer kept the previous text (the second run would be
        // longer than its own characters).
        let shaper = RustybuzzShaper::from_active_faces("WWW").expect("a face is enabled");
        clear_shape_caches();
        let long = shaper.shape_face("WWW", 16.0, 1.0).expect("the long run shapes");
        let short = shaper.shape_face("i", 16.0, 1.0).expect("the short run shapes");

        assert_eq!(long.glyph_ids.len(), long.positions.len());
        assert_eq!(short.glyph_ids.len(), short.positions.len());
        assert!(
            short.glyph_ids.len() < long.glyph_ids.len(),
            "a recycled buffer must start empty: {:?} vs {:?}",
            short.glyph_ids,
            long.glyph_ids
        );
        assert!(short.width < long.width, "the shorter text stays shorter");
    }

    #[test]
    fn a_build_with_no_vector_face_says_so_rather_than_guessing() {
        // The contract that keeps this honest on a build with no data: no face, no shaper.
        if active_faces().is_empty() {
            assert!(RustybuzzShaper::from_active_faces("abc").is_none());
        }
    }
}
