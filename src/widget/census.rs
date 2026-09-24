// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rendering census — the measurement layer behind BLUE20 layer 1.
//!
//! # The gap this closes
//!
//! Every gate before this one was built on **declarations**: does the control
//! publish a property, does it publish an event, does it `impl Draw`. Round 58
//! produced four defects that satisfied all of those and were still invisible on
//! screen — the control was laid out off-canvas, or filled with the window's own
//! colour. None of those is a declaration defect; all of them are **rendering**
//! defects, and none was detectable by asking the control what it declares.
//!
//! So this module renders each control into an in-memory raster and counts what
//! was actually painted. It answers the only question a user can see the answer
//! to: *did anything appear, and is it distinguishable from what is behind it.*
//!
//! # What it measures
//!
//! Per control, per appearance (light / dark):
//!
//! | Field | Meaning | Defect it catches |
//! |---|---|---|
//! | [`AppearanceCensus::non_background`] | pixels differing from the frame fill | control painted nothing (laid out off-canvas) |
//! | [`AppearanceCensus::dominant`] | modal painted colour | control painted, but in the background's colour |
//! | [`AppearanceCensus::detail`] | painted pixels not of the dominant colour | text/border/icon presence |
//!
//! Light and dark are compared on the dominant colour: a control whose chrome is
//! hardcoded renders identically in both, which is the whole defect.
//!
//! # Traversal unit
//!
//! By **canonical name**, never by `WidgetKind`. 13 kinds are shared by 2–5
//! controls (`ToolButton` serves both `tool_button` and `split_button`), so a kind
//! sweep silently skips 19 controls. Enumerating from
//! [`WidgetFactory::widget_names`] makes the set impossible to under-count.

use crate::compat::Vec;
use crate::core::{Color, Rect, Size};
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use crate::theme::{global_theme_manager, AppearanceMode};
use crate::widget::{draw_bridge::draw_of, Widget, WidgetFactory};

/// Geometry every control is rendered into by the census.
///
/// Roomy on purpose: a control that needs a left indicator, a label and a
/// right-hand affordance must have space for all three, or "paints nothing" would
/// be an artifact of the box rather than a fact about the control. A control that
/// only paints when it is 200 px wide is still reported as painting nothing here,
/// which is the honest reading of a 240 px box.
pub const CENSUS_RECT: Rect = Rect { x: 0, y: 0, width: 240, height: 120 };

/// Text passed to every constructor, so label-bearing controls have something to
/// paint.
pub const CENSUS_TEXT: &str = "Sample";

/// Ink measured for one control in one appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceCensus {
    /// Pixels whose RGB differs from the frame's fill colour.
    pub non_background: u32,
    /// The most common painted RGBA quadruple, or `None` when nothing was painted.
    ///
    /// Alpha is part of the identity: a theme that varies a layer's opacity rather than its hue
    /// (a scrim is the standard case) changes the drawing, and a key that dropped alpha could not
    /// see it. See [`count_ink`].
    pub dominant: Option<(u8, u8, u8, u8)>,
    /// Painted pixels that are not the dominant colour: text, borders, icons.
    pub detail: u32,
}

impl AppearanceCensus {
    /// P1: the control painted at least one pixel distinguishable from its
    /// background.
    ///
    /// This is the assertion behind round 58's defect A — a control subtree
    /// translated off-canvas renders exactly zero such pixels.
    pub fn paints_anything(&self) -> bool {
        self.non_background > 0
    }

    /// P2: the control's dominant colour is not its background.
    ///
    /// Distinct from [`Self::paints_anything`]: a control can fill its whole rect
    /// with the window colour, which paints plenty of pixels and still shows the
    /// user nothing. That was round 58's defect D.
    ///
    /// A translucent dominant is composited over `background` before the comparison, because that
    /// blend is what the user sees: a scrim stored as `rgba(0,0,0,82)` over a white page is a
    /// visibly grey rectangle, not "black, therefore different in every appearance". Comparing the
    /// raw RGB would report a translucent layer as visible against any surface, which is the
    /// false-pass direction this judgement exists to prevent.
    pub fn dominant_differs_from(&self, background: Color) -> bool {
        match self.dominant {
            Some((r, g, b, a)) => {
                let seen = if a == 255 {
                    Color::rgba(r, g, b, 255)
                } else {
                    // `background.blend(ink, alpha)` = `background * (1 - alpha) + ink * alpha`,
                    // which is the source-over composite: the dominant layer drawn on the surface.
                    background.blend(&Color::rgba(r, g, b, a), a as f32 / 255.0)
                };
                (seen.r, seen.g, seen.b) != (background.r, background.g, background.b)
            }
            None => false,
        }
    }
}

/// Both appearances for one control, plus the derived P3 verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCensus {
    /// The factory's canonical name for the control.
    pub name: &'static str,
    /// Census under the light appearance, composited over the probe colour.
    pub light: AppearanceCensus,
    /// Census under the dark appearance, composited over the probe colour.
    pub dark: AppearanceCensus,
    /// Census under the light appearance, composited over the theme background.
    ///
    /// P2 reads this, not [`Self::light`]: the question "is this control visible
    /// where it actually sits" only makes sense over the surface it sits on. The
    /// probe render from [`Self::light`] answers the different question "did it
    /// paint at all".
    pub light_on_surface: AppearanceCensus,
    /// The theme background the light surface render was composited over.
    pub light_background: Color,
    /// The theme background the dark surface render was composited over.
    pub dark_background: Color,
}

impl ControlCensus {
    /// P3: the control renders differently in the two appearances.
    ///
    /// Compared on the dominant colour, not the whole raster. Comparing rasters
    /// would fail a control that has one constant data pixel and otherwise themed
    /// chrome; comparing dominant colours fails a control whose *chrome* is
    /// hardcoded, which is the defect. The data-colour exemption table in
    /// `tools/control_color_exemptions.txt` records the controls for which an
    /// invariant dominant colour is intended rather than a defect.
    pub fn differs_between_appearances(&self) -> bool {
        self.light.dominant != self.dark.dominant
    }

    /// P2: the control is distinguishable from the surface it sits on.
    ///
    /// Read from the **surface** render, not the probe render: a control that fills
    /// its rect with the window's colour paints plenty of pixels over a probe yet is
    /// invisible in the window, which is the whole point of this check (round 58
    /// defect D).
    ///
    /// Returns `false` for two distinct situations, and the caller reports which:
    ///
    /// * the control painted nothing over the surface — either it paints nothing at
    ///   all (P1's concern) or it painted exactly the surface colour;
    /// * it painted, and its dominant colour is the surface colour.
    ///
    /// Both are "the user cannot see it", which is what this judgement is for.
    pub fn visible_against_its_surface(&self) -> bool {
        self.light_on_surface.dominant_differs_from(self.light_background)
    }
}

/// Counts ink in a rendered frame.
///
/// `background` is the colour the frame was filled with before drawing, so
/// "painted" is exactly "differs from the fill". Alpha-zero pixels are skipped:
/// they are holes, not ink.
///
/// # Why the histogram key is RGBA and not RGB
///
/// The frame buffer carries the backend's own pixels, **not** a composite: a fill at
/// `fill_alpha` lands as `(r, g, b, alpha)` with the RGB still the fill's. Keying on RGB alone
/// therefore collapses every translucent layer of the same hue into one entry — a modal scrim at
/// `rgba(0, 0, 0, 0.32)` and the same scrim at `rgba(0, 0, 0, 0.51)` were both recorded as
/// `0,0,0`, so the light/dark comparison in [`crate::widget::census::ControlCensus::differs_between_appearances`]
/// reported a themed scrim as theme-blind. That is a measurement artifact, not a finding: the two
/// SVG documents for the control differ in exactly the way the theme intended. Including alpha is
/// what makes "the drawing changed" mean something for a value a theme varies in its alpha
/// channel, which is how every platform expresses a scrim.
fn count_ink(frame: &[u8], background: Color) -> AppearanceCensus {
    let bg = (background.r, background.g, background.b);
    let mut histogram: crate::compat::HashMap<(u8, u8, u8, u8), u32> =
        crate::compat::HashMap::new();
    let mut non_background = 0u32;

    for px in frame.chunks_exact(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        if a == 0 {
            continue;
        }
        // A translucent pixel is over the fill, so it is "ink" even when its stored RGB equals the
        // fill's: the user sees the blended result, which differs from the fill. Opaque pixels keep
        // the original test.
        let painted = if a == 255 { (r, g, b) != bg } else { true };
        if painted {
            non_background += 1;
            *histogram.entry((r, g, b, a)).or_insert(0) += 1;
        }
    }

    let dominant = histogram.iter().max_by_key(|(_, count)| **count).map(|(rgba, _)| *rgba);
    let dominant_count = dominant.map(|rgba| histogram[&rgba]).unwrap_or(0);

    AppearanceCensus {
        non_background,
        dominant,
        detail: non_background.saturating_sub(dominant_count),
    }
}

/// The colour a control is composited over to measure its ink.
///
/// Deliberately **not** any theme's background. Filling with the theme background
/// made a control that legitimately paints that background (a window, whose client
/// area *is* the background) indistinguishable from one that painted nothing:
/// both produced zero "non-background" pixels, so the window looked broken and the
/// census could not say why.
///
/// A fixed probe colour removes the ambiguity. A control that covers any of this
/// magenta has painted, whatever colour it chose — the question "did ink appear"
/// no longer depends on which colour the theme happens to use. P2 still compares
/// the control's dominant colour against the **theme's** background, which is the
/// judgement it actually needs to make; only the *measurement* is over the probe.
pub const CENSUS_PROBE_BACKGROUND: Color = Color { r: 255, g: 0, b: 255, a: 255 };

/// Renders one control into a fresh surface filled with `fill` and counts ink.
///
/// The widget is drawn through its own `Draw` implementation against a
/// [`SoftwarePaintBackend`], reached via [`draw_of`] — the same bridge the real
/// render loop uses — so the census measures the actual render path rather than a
/// parallel one. A control that answers `None` from that bridge is reported as
/// painting nothing, which is exactly what mounting it would look like.
///
/// `fill` is the probe colour for the "did it paint" reading and the active theme's
/// background for the "is it visible where it sits" reading.
fn render_one(widget: &mut dyn Widget, fill: Color) -> AppearanceCensus {
    let mut backend = SoftwarePaintBackend::new(
        Size { width: CENSUS_RECT.width, height: CENSUS_RECT.height },
        1.0,
    );
    backend.begin_frame(fill);
    if let Some(drawable) = draw_of(widget) {
        let mut ctx = RenderContext::new(&mut backend);
        drawable.draw(&mut ctx);
    }
    backend.end_frame();
    count_ink(backend.frame_rgba(), fill)
}

/// The background the active theme declares.
fn active_background() -> Color {
    global_theme_manager()
        .current_theme()
        .map(|theme| theme.colors.background)
        .unwrap_or(Color::WHITE)
}

/// Selects an appearance on the process-wide manager.
fn select(appearance: AppearanceMode) {
    global_theme_manager().set_appearance(appearance);
}

/// Registers the built-in light and dark presets so [`select`] can find them.
///
/// A caller that has already registered themes of its own keeps them; this only
/// guarantees the two presets exist, which is what makes the light/dark comparison
/// meaningful in a bare process.
pub fn install_preset_appearances() {
    let mut manager = global_theme_manager();
    manager.register_theme(crate::theme::Theme::default());
    manager.register_theme(crate::theme::Theme::dark());
}

/// Renders every published control in both appearances.
///
/// The set is [`WidgetFactory::widget_names`], so a control that is registered but
/// cannot be constructed is the only way for the result to be shorter than the
/// registry — and that case is reported on stderr rather than dropped, because a
/// name that cannot be built is itself a finding.
///
/// # Theme selection inside the loop
///
/// The manager lock is taken and released around each construction, never held
/// across a `draw`. A `Draw` implementation resolves its style from the same
/// manager, so holding the lock across the call would deadlock.
pub fn census_all_controls() -> Vec<ControlCensus> {
    let factory = WidgetFactory::new_with_defaults();
    let mut results = Vec::new();

    for name in factory.widget_names() {
        let mut widget = match factory.create(name, CENSUS_RECT, CENSUS_TEXT) {
            Some(widget) => widget,
            None => {
                // Not silent: a registered name the factory cannot build is a
                // finding about the registry, and stderr keeps it visible without
                // aborting the sweep.
                log::warn!("rendering census: {name} is registered but could not be constructed");
                continue;
            }
        };

        // Give the data-bearing controls their content **before** the theme is applied.
        //
        // # Why the census wants the content at all
        //
        // `AppearanceCensus::detail` is documented as "text/border/icon presence" — a *proxy* for
        // "this control has something in it". An empty table satisfies P1 (its frame is paint) and
        // reports `detail` from its border alone, so the baseline recorded a number that could not
        // distinguish "the table drew its rows" from "the table drew its frame". Filling the sample
        // data makes that column measure the thing it is named after.
        //
        // # Why the order is content-then-theme and not theme-then-content
        //
        // `apply_active_theme` asks the control for its `widget_state()` and resolves
        // `"<kind>:<state>"`, so the state has to be the state that will be *drawn*. Filling the
        // content afterward (the previous order) left a `check_box` themed as `Normal` while it was
        // then drawn `Checked`: the light pass therefore painted the unchecked `Input` field
        // (`180,180,180`) and the dark pass painted the checked `primary` (`100,181,246`), so P3's
        // "light ≠ dark" comparison was actually comparing *unchecked* against *checked* and could
        // not see the theme at all. The two passes now differ only by appearance, which is the
        // property the judgement is named after. Exactly the ordering `theme::apply` documents for
        // the SVG exporter (see the note beside it): a state the theme keys on must be set first.
        #[cfg(full_widgets)]
        crate::widget::sample_fill::apply(name, widget.as_mut());

        select(AppearanceMode::Light);
        let light_background = active_background();
        crate::theme::apply_active_theme(&mut *widget);

        let light = render_one(&mut *widget, CENSUS_PROBE_BACKGROUND);
        // Second reading of the same theme/instance, composited over the surface the
        // control actually sits on. This is what P2 needs: a control that fills with
        // the window colour is invisible over the window and obvious over a probe,
        // and only the former is a user-visible defect.
        let light_on_surface = render_one(&mut *widget, light_background);

        // The same instance is re-themed and re-rendered. Re-creating it instead
        // would measure the constructor twice rather than measure the *theme
        // switch*, which is the property under test (round 58 defect C).
        select(AppearanceMode::Dark);
        let dark_background = active_background();
        crate::theme::apply_active_theme(&mut *widget);
        let dark = render_one(&mut *widget, CENSUS_PROBE_BACKGROUND);

        results.push(ControlCensus {
            name,
            light,
            dark,
            light_on_surface,
            light_background,
            dark_background,
        });
    }

    results
}

/// Formats a census row for the baseline file and the human-facing probe.
///
/// One line per control, fixed column order, so a change is a line diff:
/// `name ink_light dominant_light dominant_dark differs detail_light`.
pub fn format_row(row: &ControlCensus) -> crate::compat::String {
    crate::compat::format!(
        "{:<28} {:>8} {:>14} {:>14} {:>7} {:>8}",
        row.name,
        row.light.non_background,
        format_rgb(row.light.dominant),
        format_rgb(row.dark.dominant),
        if row.differs_between_appearances() { "yes" } else { "NO" },
        row.light.detail,
    )
}

/// Formats an optional RGBA quadruple, or `none` when nothing was painted.
///
/// The alpha is printed only when it is not opaque, so an ordinary control's column reads exactly
/// as it always did (`r,g,b`) and a translucent layer shows the channel that distinguishes it
/// (`r,g,b@a`). Printing the alpha unconditionally would change every line of the baseline file for
/// a value that is 255 almost everywhere, which would hide the lines that really moved.
pub fn format_rgb(rgba: Option<(u8, u8, u8, u8)>) -> crate::compat::String {
    match rgba {
        Some((r, g, b, 255)) => crate::compat::format!("{r},{g},{b}"),
        Some((r, g, b, a)) => crate::compat::format!("{r},{g},{b}@{a}"),
        None => crate::compat::String::from("none"),
    }
}
