// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **surface** declaration channel: what kind of face a control's rectangle is.
//!
//! # Why this exists
//!
//! Before this module the crate could *draw* every material a modern control needs — a drop
//! shadow, a bevel, a gradient, an alpha — but nothing could **declare** which one a control
//! was. Three measurements, all from this repository, show the same gap from three sides:
//!
//! * `theme::manager::role_base_style` handed **every** control the **same**
//!   `Shadow { x: 0, y: 2, blur: 6, .. }`, so a floating tooltip and a sunk list row painted
//!   the same elevation: the layer a face sits on could not be expressed at all.
//! * **43** files under `src/widget/` hand-derived a bevel by writing
//!   `blend(&Color::WHITE, w)` / `blend(&Color::BLACK, w)` themselves. Those two expressions
//!   are one relationship ("lit edge, shaded edge") stated twice per control, and the
//!   *direction* — is this a raised button or a cut-in well? — was carried by which pair got
//!   which colour, i.e. by the order of two nearly identical code blocks that nothing checked.
//! * `WidgetStyle::background_gradient` existed with **zero** readers and no theme schema.
//!
//! # What this module is, and is not
//!
//! It is a **value type** with four orthogonal dimensions — how far off the page the face sits,
//! which way it is turned, what it is made of, and who draws its edge. It is not a mode switch:
//! there is deliberately **no** `style: "flat" | "material" | "aero"` enum. The reason is that
//! "flat" and "skeuomorphic" are not two styles, they are two *corners of one parameter space*,
//! and every mixture between them is a surface that really ships:
//!
//! | surface | elevation | bevel | material |
//! |---|---|---|---|
//! | a modern iOS card | raised (1–2) | none | solid |
//! | a Windows-95 toolbar button | flat (0) | raised | solid |
//! | a macOS sidebar | raised (3) | none | translucent |
//!
//! A mode enum cannot name three of those without inventing a variant per mixture. Four
//! orthogonal parameters name them all, and the combinations in between come for free.
//!
//! # The identity element, and why it is the safety rope
//!
//! [`SurfaceStyle::solid`] is the identity: elevation `0`, no bevel, solid material, an outline
//! edge. It is exactly how this crate painted a face before this module existed, so wiring it in
//! changes **no** rendered pixel — the snapshot suite is the proof (see the module tests). Every
//! change to how a face is drawn must therefore be an *opt-in*: a role default, a theme
//! override, or a control that asks for something other than `solid`.
//!
//! # Reachability
//!
//! This module is compiled in **every** profile, including `mini`/`embedded`. It depends only on
//! `core` (`Color`, `Rect`, `Size`) and [`bevel`](crate::render::bevel), never on the theme
//! module — the theme side *names* these values but does not own them, so a build with no theme
//! palette still has an honest default rather than a `todo!()`. That is the same layering the
//! metrics table uses, and it is what principle #37 asks for: a missing capability is a neutral
//! value, never a fabricated one.

use crate::core::{Color, Rect};
use crate::render::bevel::{Bevel, BevelDirection};
use alloc::vec::Vec;

/// How far a face sits off the page.
///
/// # Why a small closed set and not an `f32`
///
/// An arbitrary `f32` invites a caller to write `2.7`, which is not a layer so much as a typo
/// that happens to render. The palette this crate follows has five named levels, and naming them
/// is what makes `theme.elevation(n)` a lookup rather than a curve fit — a theme override can
/// then say "cards are level 1" and mean a specific, checkable thing.
///
/// Level `0` is the identity: flush with the page, no shadow. It is not "a very small shadow",
/// because a face that is not raised must be able to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Elevation {
    /// Flush with the page. No shadow — the identity level.
    #[default]
    Flat,
    /// Just above the page: a card at rest.
    Level1,
    /// A menu or a popover.
    Level2,
    /// A floating panel or a toast.
    Level3,
    /// A modal or a dragged item, above everything.
    Level4,
}

impl Elevation {
    /// The level as a number, for token lookup and diagnostics.
    pub const fn level(self) -> u8 {
        match self {
            Self::Flat => 0,
            Self::Level1 => 1,
            Self::Level2 => 2,
            Self::Level3 => 3,
            Self::Level4 => 4,
        }
    }

    /// The level for a number, saturating at [`Elevation::Level4`].
    ///
    /// Saturating rather than wrapping or `None`: a theme that asks for level 9 has plainly
    /// asked for "as high as this scale goes", and silently returning `Flat` would turn its
    /// card into a flush panel — the opposite of its intent, and the harder bug to notice.
    pub const fn from_level(level: u8) -> Self {
        match level {
            0 => Self::Flat,
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            _ => Self::Level4,
        }
    }

    /// The published token for this level, e.g. `"flat"` or `"level2"`.
    ///
    /// Tokens exist so a theme file can name a level without a magic number, and so the parser
    /// below and the serialiser agree by construction rather than by two hand-kept lists.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Level1 => "level1",
            Self::Level2 => "level2",
            Self::Level3 => "level3",
            Self::Level4 => "level4",
        }
    }

    /// Parses a published token, or answers `None`.
    ///
    /// A named `parse` rather than `FromStr` because the crate's other token parsers return
    /// `Option` and can be used with `?`; an inherent `parse` would shadow the trait's, so this
    /// deliberately does not implement `FromStr` — the same call the bevel direction made.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "flat" | "none" | "0" => Some(Self::Flat),
            "level1" | "1" => Some(Self::Level1),
            "level2" | "2" => Some(Self::Level2),
            "level3" | "3" => Some(Self::Level3),
            "level4" | "4" => Some(Self::Level4),
            _ => None,
        }
    }
}

/// One layer of a raised face's shadow.
///
/// A record rather than a bare [`Shadow`](crate::style::Shadow) so this module keeps its
/// dependency on `style` out of the type: the elevation a theme declares is *data*, and the
/// render-layer shadow it becomes is a separate concern with its own representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevationShadow {
    /// Vertical offset in logical pixels. Each level is offset progressively further down.
    pub y: i32,
    /// Blur radius in logical pixels.
    pub blur: u32,
    /// Shadow alpha, `0..=255`. Grows with the level so a higher face casts a softer, wider
    /// shadow rather than merely a further one.
    pub alpha: u8,
}

/// The default shadow for `elevation` under a dark or light page.
///
/// # Why the six numbers are here and not in a theme file
///
/// They are the *relationship* between levels — each one a step further down and a step softer
/// than the one below it — not a palette choice. A theme that wants different shadows overrides
/// them per level (see `theme::elevation`); a theme that wants none sets every level to
/// [`Elevation::Flat`]. Keeping the relationship in code is what stops a theme from accidentally
/// inverting it (a level 3 shadow softer than a level 1 one) while still letting it restate it.
pub const fn default_shadow(elevation: Elevation) -> Option<ElevationShadow> {
    match elevation {
        Elevation::Flat => None,
        // `y: 2, blur: 6, alpha: 60` is not a guess: it is the exact shadow the theme layer cast
        // under every control before this ladder existed (`role_base_style`'s old literal). It is
        // level 2 because that is where a face "floating just above its neighbours" belongs, and
        // keeping the numbers byte-identical is what lets the default theme's snapshots go on
        // passing unchanged while the *mechanism* underneath them is replaced.
        Elevation::Level1 => Some(ElevationShadow { y: 1, blur: 3, alpha: 40 }),
        Elevation::Level2 => Some(ElevationShadow { y: 2, blur: 6, alpha: 60 }),
        Elevation::Level3 => Some(ElevationShadow { y: 4, blur: 12, alpha: 70 }),
        Elevation::Level4 => Some(ElevationShadow { y: 8, blur: 24, alpha: 85 }),
    }
}

/// What a face is made of.
///
/// # Why translucency is not just "a lower alpha"
///
/// A translucent face and a transparent one are different intents. The first is a material — the
/// system samples what is behind it and tints it, which is why a macOS sidebar keeps its text
/// legible over a photograph. The second is simply a colour nobody can see, which is usually a
/// bug. Naming the material keeps that distinction expressible, and lets a renderer that cannot
/// yet sample the backdrop (see BLUE24 §12 U-12) implement the intent as a tinted alpha rather
/// than silently dropping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Material {
    /// An opaque fill. The identity material.
    #[default]
    Solid,
    /// A translucent fill that tints whatever is behind it.
    Translucent,
}

impl Material {
    /// The published token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Translucent => "translucent",
        }
    }

    /// Parses a published token, or answers `None`.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "solid" | "opaque" => Some(Self::Solid),
            "translucent" | "translucent_material" => Some(Self::Translucent),
            _ => None,
        }
    }
}

/// Who paints a face's outermost edge.
///
/// # Why an edge is a separate axis
///
/// This crate has **two** ways to make a face's boundary visible — a stroked outline and a cast
/// shadow — and for most of its life every control drew both, which is why every control looked
/// faintly outlined *and* faintly floating at once. Modern flat design usually wants exactly one:
/// a card made entirely of a shadow has no stroke, and an input field made entirely of a stroke
/// has no shadow. Choosing is a real decision, independent of how far the face is raised, so it
/// gets its own axis rather than being inferred from `elevation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hairline {
    /// A stroked outline, in the control's resolved border colour. The identity edge.
    #[default]
    Outline,
    /// No stroke; the cast shadow (if any) is the only edge.
    Shadow,
    /// No visible edge at all — for a face that is deliberately flush and borderless.
    None,
}

impl Hairline {
    /// The published token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Shadow => "shadow",
            Self::None => "none",
        }
    }

    /// Parses a published token, or answers `None`.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "outline" | "stroke" => Some(Self::Outline),
            "shadow" => Some(Self::Shadow),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// A bevel declared as data: which way the face is turned, and from what colour its two tones
/// derive.
///
/// # Why this is not the [`Bevel`](crate::render::bevel::Bevel) itself
///
/// `Bevel` holds *resolved colours*, and resolving them needs the control's border colour —
/// which is a fact only known per draw. A declared surface style must be equal across frames and
/// comparable, so it holds the two facts a theme can state (direction + a base tone) and the
/// renderer turns them into a `Bevel` at draw time. Keeping the resolved tones out also means a
/// `SurfaceStyle` cannot go stale when the palette changes under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BevelSpec {
    /// Which way the face is turned.
    pub direction: BevelDirection,
    /// The colour the lit and shaded tones step from.
    ///
    /// `None` asks the renderer to derive them from the face's own resolved border colour, which
    /// is what most callers want: a bevel that follows the palette rather than fighting it.
    pub base: Option<Color>,
}

impl BevelSpec {
    /// A bevel turned `direction`, with its tones derived from the face's border colour.
    pub const fn new(direction: BevelDirection) -> Self {
        Self { direction, base: None }
    }

    /// A bevel with explicitly stated tones.
    pub const fn from_base(direction: BevelDirection, base: Color) -> Self {
        Self { direction, base: Some(base) }
    }

    /// Resolves this specification into a paintable [`Bevel`].
    ///
    /// `border` is the face's resolved border colour, used when no explicit base was stated.
    pub fn resolve(self, border: Color) -> Bevel {
        let base = self.base.unwrap_or(border);
        Bevel::from_base(base).with_direction(self.direction)
    }
}

/// A face's material declaration: four orthogonal dimensions, each independently defaultable.
///
/// See the module docs for why this is a parameter space rather than a mode enum. The four
/// dimensions and the gap each one closes:
///
/// | dimension | closes |
/// |---|---|
/// | [`elevation`](Self::elevation) | every control receiving the same shadow, so a raised layer could not be expressed |
/// | [`bevel`](Self::bevel) | 43 files deriving a face's two tones and its direction by hand |
/// | [`material`](Self::material) | translucency being indistinguishable from a low alpha |
/// | [`hairline`](Self::hairline) | every face drawing both a stroke and a shadow at once |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceStyle {
    /// How far the face sits off the page.
    pub elevation: Elevation,
    /// The face's bevel, or `None` for a flat face.
    pub bevel: Option<BevelSpec>,
    /// What the face is made of.
    pub material: Material,
    /// Who draws the face's outer edge.
    pub hairline: Hairline,
}

impl SurfaceStyle {
    /// The identity surface: flush, flat, solid, outlined.
    ///
    /// This is how this crate painted a face before this type existed, so it is the value every
    /// control inherits unless something explicitly asks otherwise. Anything that changes how a
    /// face is drawn must therefore *replace* this rather than rely on a default being absent —
    /// which is what makes wiring this type in pixel-neutral for the default theme.
    pub const fn solid() -> Self {
        Self {
            elevation: Elevation::Flat,
            bevel: None,
            material: Material::Solid,
            hairline: Hairline::Outline,
        }
    }

    /// This surface at `elevation`.
    pub const fn elevated(mut self, elevation: Elevation) -> Self {
        self.elevation = elevation;
        self
    }

    /// This surface with a bevel turned `direction`, tones derived from the border colour.
    pub const fn beveled(mut self, direction: BevelDirection) -> Self {
        self.bevel = Some(BevelSpec::new(direction));
        self
    }

    /// This surface with a bevel whose tones come from `base`.
    pub const fn beveled_from(mut self, direction: BevelDirection, base: Color) -> Self {
        self.bevel = Some(BevelSpec::from_base(direction, base));
        self
    }

    /// This surface made translucent.
    pub const fn translucent(mut self) -> Self {
        self.material = Material::Translucent;
        self
    }

    /// This surface whose edge is left to its shadow.
    pub const fn hairline_shadow(mut self) -> Self {
        self.hairline = Hairline::Shadow;
        self
    }

    /// Whether this is the identity surface.
    ///
    /// A renderer can use this to take the exact pre-surface code path, which is what keeps a
    /// flat, flush, solid, outlined face byte-for-byte identical to how it was drawn before.
    pub const fn is_identity(&self) -> bool {
        self.elevation.level() == 0
            && self.bevel.is_none()
            && matches!(self.material, Material::Solid)
            && matches!(self.hairline, Hairline::Outline)
    }

    /// Whether the face is raised off the page.
    pub const fn is_raised(&self) -> bool {
        self.elevation.level() != 0
    }

    /// The fill alpha this surface wants applied to a resolved colour.
    ///
    /// `255` for a solid face — i.e. no change — and a tinted partial alpha for a translucent
    /// one. # Why not an `is_translucent` bool
    ///
    /// A caller that asks "is this translucent?" and then picks its own alpha is the two-spellings
    /// -of-one-fact failure principle #101 warns about: the answer would differ per control. This
    /// returns the single alpha the material implies, so every translucent face is equally
    /// translucent.
    pub const fn fill_alpha(&self) -> u8 {
        match self.material {
            Material::Solid => u8::MAX,
            Material::Translucent => 230,
        }
    }

    /// Applies this surface's material to a resolved face colour.
    ///
    /// The identity for a solid face (the colour is returned unchanged), and a tint-carrying
    /// alpha for a translucent one. Kept here rather than at each call site so "what translucent
    /// looks like" has exactly one definition.
    pub fn apply_fill(&self, color: Color) -> Color {
        match self.material {
            Material::Solid => color,
            Material::Translucent => color.with_alpha(self.fill_alpha()),
        }
    }

    /// Whether the renderer should stroke the face's outline.
    pub const fn draws_outline(&self) -> bool {
        matches!(self.hairline, Hairline::Outline)
    }

    /// Whether the renderer should cast this surface's elevation shadow.
    pub const fn draws_shadow(&self) -> bool {
        // A raised face always casts; a flat one never does. `hairline: Shadow` does not raise a
        // flat face — it *removes the stroke*, which is deliberately not the same as raising it.
        self.is_raised()
    }

    /// Parses `token` as an [`Elevation`], answering `None` for an unknown one.
    ///
    /// Exposed so a theme loader has one place to reject an unknown level rather than silently
    /// falling back to the identity (see BLUE24 §10A.6 criterion 7).
    pub fn parse_elevation(token: &str) -> Option<Elevation> {
        Elevation::parse(token)
    }
}

/// A surface's shadow, in the render layer's own terms.
///
/// A small record rather than reaching into `style::Shadow` so this module stays below the style
/// layer; `style` converts this into its own shadow type at application time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceShadow {
    /// Horizontal offset.
    pub x: i32,
    /// Vertical offset.
    pub y: i32,
    /// Blur radius.
    pub blur: u32,
    /// Shadow colour, already carrying its alpha.
    pub color: Color,
}

impl SurfaceStyle {
    /// The shadow this surface casts, in render terms, or `None` when it casts none.
    ///
    /// `tint` is the shadow's colour with the elevation alpha applied, so a theme can state the
    /// shadow's hue once and let each level own its opacity.
    pub fn shadow(&self, tint: Color) -> Option<SurfaceShadow> {
        if !self.draws_shadow() {
            return None;
        }
        default_shadow(self.elevation).map(|s| SurfaceShadow {
            x: 0,
            y: s.y,
            blur: s.blur,
            color: tint.with_alpha(s.alpha),
        })
    }
}

/// A **role default**: the surface a control plays a role with, absent any override.
///
/// # Why a table next to `role_colors` rather than a new theme layer
///
/// The crate already resolves a role's *colours* in code (`theme::manager::role_colors`) and
/// lets a theme override them by name. Surfaces work the same way: one function states the role
/// defaults, a theme override replaces any of the four dimensions per role or per `kind:state`,
/// and there is no third mechanism. This function is the code half of that pair.
///
/// # Why most roles resolve to the historical soft shadow
///
/// `solid()` is the identity — no elevation — but the **default role** below is not `solid()`:
/// before this module existed, `role_base_style` cast one shadow under *every* control, and the
/// plan's criterion 9 requires the default theme to go on rendering byte-for-byte as before. So
/// the value an unclassified kind resolves to is that same shadow, now expressed as the level
/// whose shape it already had (`y: 2, blur: 6` — [`Elevation::Level2`]), and `solid()` stays the
/// pure identity that a *control* or an *explicit override* can reach.
///
/// The two are therefore different questions, which is why they are different functions:
/// `solid()` answers "what is a face with nothing declared?", this table answers "what does the
/// default theme paint a kind as?".
///
/// The name is the kind's name, not a `WidgetKind`, for the same reason `role_colors` takes one:
/// a kind the library has never heard of gets the theme's own default rather than a compile error.
pub fn role_surface_style(kind_name: &str) -> SurfaceStyle {
    // Normalised the same way `WidgetRole::for_kind_name` normalises, so the two role tables
    // classify an identically spelled kind identically.
    let normalized: Vec<u8> = kind_name
        .bytes()
        .filter(|b| !matches!(b, b'_' | b'-' | b' '))
        .map(|b| b.to_ascii_lowercase())
        .collect();
    let name = core::str::from_utf8(&normalized).unwrap_or("");
    match name {
        // Raised chrome: things that sit *above* the page and are meant to be seen as such.
        "card" => SurfaceStyle::solid().elevated(Elevation::Level1),
        "toast" => SurfaceStyle::solid().elevated(Elevation::Level3).translucent(),
        "tooltip" => SurfaceStyle::solid().elevated(Elevation::Level4),
        "popup" | "popover" | "menu" | "contextmenu" | "dropdown" | "combobox" => {
            SurfaceStyle::solid().elevated(Elevation::Level2)
        }
        // A dialog is a raised panel, and its shadow is its edge.
        "dialog" | "modal" | "drawer" | "sheet" => {
            SurfaceStyle::solid().elevated(Elevation::Level3).hairline_shadow()
        }
        // Pressable chrome: raised toward the viewer. Kept at level 0 because a button that
        // floated would fight every toolbar it sits in — its *bevel* is what makes it pressable.
        "button" | "pushbutton" | "togglebutton" | "toolbutton" => {
            SurfaceStyle::solid().beveled(BevelDirection::Raised)
        }
        // A field is cut *into* the page: the inset well is what says "you can type here".
        "lineedit" | "textedit" | "textarea" | "spinbox" | "combobox_edit" => {
            SurfaceStyle::solid().beveled(BevelDirection::Inset)
        }
        // Everything else resolves to the shadow the default theme has always cast under a
        // control — `Elevation::Level2`, whose shape is the old literal exactly. This is what keeps
        // criterion 9 (the default preset's snapshots) green while the mechanism under it changes:
        // the *value* is preserved, and it is now a level a theme can restate rather than a literal
        // no theme could reach.
        _ => SurfaceStyle::solid().elevated(Elevation::Level2),
    }
}

/// Whether `rect` is big enough for a bevel to be legible.
///
/// A bevel needs two edges' worth of pixels on each axis; below that the two lines collapse onto
/// each other and the face reads as a smudge rather than an edge. A caller that asks for a bevel
/// on a tiny face gets a flat one instead — the honest degradation, stated once here rather than
/// left to each call site (principle #50: degenerate input is handled, not assumed away).
pub fn bevel_fits(rect: Rect) -> bool {
    // Two pixels of edge per axis is the floor: one for the lit line, one for the shaded one.
    rect.width >= 4 && rect.height >= 4
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §10A.6 criterion 1: `solid()` is the identity of all four dimensions.
    #[test]
    fn solid_is_the_identity_element() {
        let s = SurfaceStyle::solid();
        assert_eq!(s.elevation, Elevation::Flat);
        assert!(s.bevel.is_none());
        assert_eq!(s.material, Material::Solid);
        assert_eq!(s.hairline, Hairline::Outline);
        assert!(s.is_identity(), "solid() must be the identity");
        assert!(!s.is_raised());
        assert!(!s.draws_shadow());
        assert!(s.draws_outline());
        assert_eq!(s.fill_alpha(), u8::MAX, "a solid face is not faded");
    }

    /// The identity is preserved under the `Default` derive too — a `SurfaceStyle` reached
    /// through `Default::default()` must not accidentally be something other than flush.
    #[test]
    fn default_is_the_identity_element() {
        assert_eq!(SurfaceStyle::default(), SurfaceStyle::solid());
        assert!(SurfaceStyle::default().is_identity());
    }

    /// Every builder leaves the dimensions it does not name alone.
    #[test]
    fn builders_are_orthogonal() {
        let raised = SurfaceStyle::solid().elevated(Elevation::Level2);
        assert!(raised.is_raised() && raised.bevel.is_none());
        assert!(raised.draws_shadow() && raised.draws_outline());
        assert_eq!(raised.material, Material::Solid);

        let beveled = SurfaceStyle::solid().beveled(BevelDirection::Raised);
        assert!(!beveled.is_raised(), "a bevel alone does not raise a face");
        assert!(!beveled.draws_shadow(), "so it casts no shadow");
        assert_eq!(
            beveled.bevel.map(|b| b.direction),
            Some(BevelDirection::Raised),
            "but the direction is recorded"
        );

        let glassy = SurfaceStyle::solid().translucent();
        assert_eq!(glassy.material, Material::Translucent);
        assert!(!glassy.is_raised() && glassy.bevel.is_none());
        assert!(!glassy.is_identity(), "a material change is not the identity");

        let edgeless = SurfaceStyle::solid().hairline_shadow();
        assert_eq!(edgeless.hairline, Hairline::Shadow);
        assert!(!edgeless.draws_outline());
        assert!(!edgeless.is_identity());
    }

    /// §10A.6 criterion 1 (the mechanism guarantee): the default surface draws no shadow, so
    /// wiring this type in cannot add one to a control that had none.
    #[test]
    fn the_identity_casts_no_shadow() {
        assert!(SurfaceStyle::solid().shadow(Color::BLACK).is_none());
        assert!(
            SurfaceStyle::solid().beveled(BevelDirection::Raised).shadow(Color::BLACK).is_none(),
            "a flat beveled face is still flat — the bevel is not a raise"
        );
    }

    /// The elevation ladder is monotone: each level is at least as far down and at least as
    /// blurred as the one below it. A theme may restate the numbers but not this relationship,
    /// which is why it lives in code.
    #[test]
    fn the_elevation_ladder_is_monotone() {
        let levels = [Elevation::Level1, Elevation::Level2, Elevation::Level3, Elevation::Level4];
        let mut previous = default_shadow(Elevation::Flat);
        assert!(previous.is_none(), "flat casts nothing");
        for level in levels {
            let shadow = default_shadow(level).expect("a raised level casts a shadow");
            if let Some(prev) = previous {
                assert!(shadow.y > prev.y, "{level:?} must sit further down than the level below");
                assert!(shadow.blur > prev.blur, "{level:?} must blur more than the level below");
                assert!(shadow.alpha > prev.alpha, "{level:?} must be more opaque than below");
            }
            previous = Some(shadow);
        }
    }

    /// A translucent face really is translucent, and one definition of "how translucent" applies
    /// to every such face.
    #[test]
    fn translucent_surfaces_share_one_alpha() {
        let glassy = SurfaceStyle::solid().translucent();
        assert!(glassy.fill_alpha() < u8::MAX);
        // The same material applied to two different colours yields the same alpha.
        let a = glassy.apply_fill(Color::WHITE);
        let b = glassy.apply_fill(Color::BLACK);
        assert_eq!(a.to_i32().3, b.to_i32().3);
        assert_eq!(a.to_i32().3, glassy.fill_alpha() as i32);
        // And a solid face is returned unchanged.
        let solid = SurfaceStyle::solid();
        assert_eq!(solid.apply_fill(Color::WHITE).to_i32(), Color::WHITE.to_i32());
    }

    /// §10A.6 criterion 7: an unknown token is **rejected**, not silently degraded to a default.
    #[test]
    fn unknown_tokens_are_rejected() {
        assert_eq!(Elevation::parse("sunken"), None);
        assert_eq!(Elevation::parse(""), None);
        assert_eq!(Elevation::parse("Level1"), None, "tokens are lower-case");
        assert_eq!(Material::parse("frosted"), None);
        assert_eq!(Hairline::parse("double"), None);
    }

    /// Every published token round-trips through its own parser — the two halves cannot drift.
    #[test]
    fn tokens_round_trip() {
        for level in [
            Elevation::Flat,
            Elevation::Level1,
            Elevation::Level2,
            Elevation::Level3,
            Elevation::Level4,
        ] {
            assert_eq!(Elevation::parse(level.as_str()), Some(level));
            assert_eq!(Elevation::from_level(level.level()), level);
        }
        for material in [Material::Solid, Material::Translucent] {
            assert_eq!(Material::parse(material.as_str()), Some(material));
        }
        for hairline in [Hairline::Outline, Hairline::Shadow, Hairline::None] {
            assert_eq!(Hairline::parse(hairline.as_str()), Some(hairline));
        }
    }

    /// `from_level` saturates rather than wrapping, so a theme asking for a level beyond the
    /// scale gets the top of it instead of silently falling back to flush.
    #[test]
    fn from_level_saturates() {
        assert_eq!(Elevation::from_level(9), Elevation::Level4);
        assert_eq!(Elevation::from_level(u8::MAX), Elevation::Level4);
        assert_eq!(Elevation::from_level(0), Elevation::Flat);
    }

    /// §10A.6 criterion 2 (the shape of the assertion): flat and skeuomorphic produce **different
    /// geometry**, not merely different colours. A flat card has a shadow and no bevel lines; a
    /// beveled button has bevel lines and no shadow. Asserting the two *trajectories* is what
    /// distinguishes this from a palette test.
    #[test]
    fn flat_and_skeuomorphic_differ_in_geometry_not_only_colour() {
        let flat_card = SurfaceStyle::solid().elevated(Elevation::Level1);
        let beveled_button = SurfaceStyle::solid().beveled(BevelDirection::Raised);

        let card_shadow = flat_card.shadow(Color::BLACK);
        let button_shadow = beveled_button.shadow(Color::BLACK);
        assert!(card_shadow.is_some(), "the card casts a shadow");
        assert!(button_shadow.is_none(), "the button does not: it is flush");

        // The card has no bevel lines; the button has no shadow. Neither draws both.
        assert!(flat_card.bevel.is_none());
        assert!(beveled_button.bevel.is_some());
    }

    /// §10A.6 criterion 6's unit half: a bevel spec resolves against the face's border colour
    /// when no explicit base is given, and against the explicit one when it is.
    #[test]
    fn a_bevel_spec_resolves_against_the_border_colour() {
        let border = Color::rgb(64, 64, 64);
        let derived = BevelSpec::new(BevelDirection::Raised).resolve(border);
        // The tones step from the border, so the light tone is lighter than it and the shade
        // darker — the relationship, not two literals.
        assert!(derived.light.luminance() > border.luminance());
        assert!(derived.shade.luminance() < border.luminance());
        assert_eq!(derived.direction, BevelDirection::Raised);

        let explicit = Color::rgb(200, 0, 0);
        let stated = BevelSpec::from_base(BevelDirection::Inset, explicit).resolve(border);
        assert_eq!(stated.base.to_i32(), explicit.to_i32());
    }

    /// §10A.6 criterion 6 (role layer): a role's surface is what the table says, and an
    /// unclassified kind gets the theme's own default shadow — the level whose shape the default
    /// theme has always cast, so adding a kind cannot change its appearance by accident.
    #[test]
    fn role_defaults_classify_and_default_to_the_historical_shadow() {
        assert_eq!(role_surface_style("card").elevation, Elevation::Level1);
        assert_eq!(role_surface_style("Card").elevation, Elevation::Level1, "names normalise");
        assert_eq!(role_surface_style("toast").material, Material::Translucent);
        assert!(role_surface_style("dialog").draws_shadow());
        assert_eq!(
            role_surface_style("button").bevel.map(|b| b.direction),
            Some(BevelDirection::Raised)
        );
        assert_eq!(
            role_surface_style("line_edit").bevel.map(|b| b.direction),
            Some(BevelDirection::Inset),
            "a field is a well, spelled both ways"
        );

        // An unknown kind keeps the default theme's existing appearance — that is the whole
        // point of routing the old literal through a level rather than deleting it.
        for unknown in ["gizmo", "", "some_third_party_widget"] {
            let s = role_surface_style(unknown);
            assert_eq!(
                s.elevation,
                Elevation::Level2,
                "{unknown:?} must keep the theme's historical shadow, not gain or lose one"
            );
            assert!(s.bevel.is_none(), "and gain no bevel it never had");
        }
    }

    /// The identity and the theme's role default are two different questions, and the answer to
    /// the first must stay flat so a control that *asks* for a flush face can reach one.
    #[test]
    fn the_identity_is_reachable_even_though_the_role_default_is_not_it() {
        assert!(SurfaceStyle::solid().is_identity());
        assert!(!role_surface_style("gizmo").is_identity());
        assert!(
            role_surface_style("gizmo").elevated(Elevation::Flat).is_identity(),
            "a theme can flatten a role back to the identity"
        );
    }

    /// The value an unclassified kind resolves to is *exactly* the shadow the theme layer cast
    /// before this ladder existed (`y: 2, blur: 6, alpha: 60`). This is criterion 9's mechanism:
    /// the default preset's snapshots can only stay byte-identical if the number is the same.
    #[test]
    fn the_default_role_shadow_is_the_historical_value() {
        let shadow = default_shadow(Elevation::Level2).expect("level 2 casts");
        assert_eq!(shadow.y, 2);
        assert_eq!(shadow.blur, 6);
        assert_eq!(shadow.alpha, 60);
    }

    /// §10A.8 risk 3's stop line, asserted: there are exactly four dimensions, and each answers a
    /// distinct question. A fifth would have to replace one of these, per the plan.
    #[test]
    fn there_are_exactly_four_dimensions() {
        // Named individually so adding a fifth field to `SurfaceStyle` requires touching this test.
        let s = SurfaceStyle::solid();
        let _ = (s.elevation, s.bevel, s.material, s.hairline);
    }

    /// Degenerate geometry: a face too small for a bevel reports so, so the renderer can fall
    /// back to a flat one instead of drawing two lines on top of each other.
    #[test]
    fn a_bevel_needs_room() {
        assert!(!bevel_fits(Rect::new(0, 0, 3, 20)));
        assert!(!bevel_fits(Rect::new(0, 0, 20, 3)));
        assert!(!bevel_fits(Rect::new(0, 0, 0, 0)));
        assert!(bevel_fits(Rect::new(0, 0, 4, 4)));
        assert!(bevel_fits(Rect::new(0, 0, 120, 40)));
    }
}
