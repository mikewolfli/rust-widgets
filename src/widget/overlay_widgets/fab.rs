// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FAB (Floating Action Button) widget — a circular floating button for
//! primary actions.
//!
//! The FAB is a Material Design-style circular button that floats above the UI.
//! It displays an icon as a text character (e.g., "+") and supports press
//! animation, shadow, and click signal emission.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The three size classes a floating action button may be drawn at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FabSize {
    /// The standard 56 px button.
    #[default]
    Regular,
    /// The 40 px mini button.
    Mini,
    /// The 96 px large button introduced with the expressive Material sizes.
    Large,
}

impl FabSize {
    /// The edge length, in logical pixels, a host should give this size class.
    pub const fn edge(self) -> u32 {
        match self {
            Self::Regular => 56,
            Self::Mini => 40,
            Self::Large => 96,
        }
    }
}

/// Floating Action Button widget.
///
/// A circular button with a background color (typically accent), an icon
/// rendered as centered text, a drop shadow, and a press animation that
/// briefly shrinks the button. Emits `clicked` on press-release.
pub struct FAB {
    base: BaseWidget,
    /// The icon text character displayed in the center (e.g. "+", "✕", "↓").
    icon_text: String,
    icon_source: Option<IconSource>,
    /// The fill color of the circular button.
    accent_color: Color,
    /// Which size class the button is drawn at.
    size_kind: FabSize,
    /// An optional label that turns the circular button into an extended capsule.
    extended_label: Option<String>,
    /// How far the press shrink has travelled, `0.0` at rest and `1.0` fully pressed.
    ///
    /// A driver rather than a multiplication of `is_pressed`: the scale is an animation, and
    /// jumping between the two ends was the whole gap this replaces. At rest the value is
    /// exactly zero, so the drawn radius is byte-identical to the un-animated button.
    press: PropertyDriver,
    /// How much of the button is visible, `0.0` hidden to `1.0` shown.
    visibility: PropertyDriver,
    /// Whether downward scrolling hides the button.
    hide_on_scroll: bool,
    /// The shadow's depth, `0.0..=8.0` (OV-07).
    ///
    /// The offset and the radius reservation used to be the literal `3.0`; a host on a dense
    /// background can now ask for a deeper or shallower drop without replacing the control.
    elevation: f32,
    /// The cached extended-label face, keyed by its size (G-A02).
    label_font_cache: Option<(u32, Font)>,
    /// The cached icon-text face, keyed by its size (G-A02).
    icon_font_cache: Option<(u32, Font)>,
    /// An optional speed dial opened by this button (OV-009).
    ///
    /// A slot rather than a hard dependency: a FAB with no dial is exactly the control it was,
    /// and the dial keeps its own action list, expansion state and signals. A build without the
    /// companion module (`full_widgets`) simply has no slot.
    #[cfg(full_widgets)]
    speed_dial: Option<Box<crate::widget::companions::SpeedDial>>,
}

impl FAB {
    /// The fill a FAB is constructed with, before any caller override.
    ///
    /// Held as a named constant because [`FAB::draw`] has to tell "the caller chose this
    /// colour" apart from "nobody has chosen yet, so read the theme". Those two cases are
    /// otherwise indistinguishable from the field alone, and treating the default as an
    /// override is what kept the control theme-blind.
    pub const DEFAULT_ACCENT_COLOR: Color = Color::PRIMARY;

    /// Creates a new FAB widget with the given geometry.
    ///
    /// Defaults: icon text "+", accent color `Color::PRIMARY`, normal size.
    ///
    /// The default is resolved against the active theme at draw time: this value is used
    /// only when no theme is active.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FAB, geometry, "FAB"),
            icon_text: String::from("+"),
            icon_source: None,
            accent_color: Self::DEFAULT_ACCENT_COLOR,
            size_kind: FabSize::Regular,
            extended_label: None,
            press: PropertyDriver::at(0.0, MotionSlot::Fast),
            visibility: PropertyDriver::at(1.0, MotionSlot::Normal),
            hide_on_scroll: false,
            elevation: 3.0,
            label_font_cache: None,
            icon_font_cache: None,
            #[cfg(full_widgets)]
            speed_dial: None,
        }
    }

    /// Sets the icon text displayed in the center of the button.
    ///
    /// Common values: `"+"`, `"✕"`, `"↓"`, `"↑"`, `"✓"`, `"✎"`.
    pub fn set_icon_text(&mut self, text: &str) {
        if self.icon_text == text && self.icon_source.is_none() {
            return;
        }
        self.icon_text = text.to_string();
        self.icon_source = None;
        self.base.request_redraw();
    }

    /// Returns the current icon text.
    pub fn icon_text(&self) -> &str {
        if self.icon_source.is_some() {
            ""
        } else {
            &self.icon_text
        }
    }

    /// Sets a built-in or static-geometry icon, replacing the legacy icon text.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        // The same typed source with no legacy text left to clear is not a change; repainting it
        // would be a no-op frame (the guard shape `set_elevation` uses).
        if self.icon_source == source && self.icon_text.is_empty() {
            return;
        }
        self.icon_text.clear();
        self.icon_source = source;
        self.base.request_redraw();
    }

    /// Returns the typed icon source, if one is active.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Sets the accent (fill) color of the circular button.
    pub fn set_accent_color(&mut self, color: Color) {
        if self.accent_color == color {
            return;
        }
        self.accent_color = color;
        self.base.request_redraw();
    }

    /// Returns the current accent color.
    pub fn accent_color(&self) -> Color {
        self.accent_color
    }

    /// Sets whether this FAB renders in mini (smaller) mode.
    ///
    /// Mini FABs are typically 40×40 instead of the standard 56×56.
    /// The geometry rect should be adjusted separately; this flag controls
    /// visual proportions like shadow offset and press scale factor.
    pub fn set_mini(&mut self, mini: bool) {
        let size_kind = if mini { FabSize::Mini } else { FabSize::Regular };
        self.set_size(size_kind);
    }

    /// Returns whether this FAB is in mini mode.
    pub fn is_mini(&self) -> bool {
        self.size_kind == FabSize::Mini
    }

    /// Returns the button's size class.
    pub fn size_kind(&self) -> FabSize {
        self.size_kind
    }

    /// Sets the button's size class.
    ///
    /// This changes the size the control reports in [`Widget::size_hint`]; a host that
    /// controls the geometry itself should also give it the class's own edge.
    pub fn set_size(&mut self, size: FabSize) {
        if self.size_kind == size {
            return;
        }
        self.size_kind = size;
        self.base.request_redraw();
    }

    /// Turns the button into an extended capsule when `label` is set, or back into a circle
    /// when it is `None`.
    pub fn set_extended_label(&mut self, label: Option<impl Into<String>>) {
        let label = label.map(Into::into);
        if self.extended_label == label {
            return;
        }
        self.extended_label = label;
        self.base.request_redraw();
    }

    /// Returns the extended label, when the button is a capsule.
    pub fn extended_label(&self) -> Option<&str> {
        self.extended_label.as_deref()
    }

    /// Enables hiding the button while the page scrolls down.
    pub fn set_hide_on_scroll(&mut self, hide: bool) {
        if self.hide_on_scroll == hide {
            return;
        }
        self.hide_on_scroll = hide;
        self.base.request_redraw();
    }

    /// Reports a scroll in wheel notches: a downward scroll hides the button, an upward one
    /// shows it again. Does nothing unless [`Self::set_hide_on_scroll`] is enabled.
    pub fn report_scroll(&mut self, delta_y: i32) {
        if !self.hide_on_scroll || delta_y == 0 {
            return;
        }
        self.visibility.set_target(if delta_y > 0 { 0.0 } else { 1.0 });
        self.base.request_redraw();
    }

    /// How much of the button is currently shown, `0.0` to `1.0`.
    pub fn visibility_progress(&self) -> f32 {
        self.visibility.value()
    }

    /// Attaches a speed dial this button opens and closes (OV-009).
    ///
    /// The slot follows the `AdaptiveScaffold` slot pattern: the host may set one, replace it,
    /// or leave it unset, and the two controls stay independently usable. The dial is laid over
    /// the button's own rectangle so its primary circle sits behind this button; the button
    /// remains the hit target and toggles the dial on a completed click.
    #[cfg(full_widgets)]
    pub fn set_speed_dial(&mut self, dial: Option<crate::widget::companions::SpeedDial>) {
        self.speed_dial = dial.map(Box::new);
        let geometry = self.geometry();
        if let Some(dial) = self.speed_dial.as_deref_mut() {
            dial.set_geometry(geometry);
            crate::theme::apply_theme_to_widget(dial);
        }
        self.base.request_redraw();
    }

    /// Returns the attached speed dial, if one is set (OV-009).
    #[cfg(full_widgets)]
    pub fn speed_dial(&self) -> Option<&crate::widget::companions::SpeedDial> {
        self.speed_dial.as_deref()
    }

    /// Returns the attached speed dial mutably, if one is set (OV-009).
    #[cfg(full_widgets)]
    pub fn speed_dial_mut(&mut self) -> Option<&mut crate::widget::companions::SpeedDial> {
        self.speed_dial.as_deref_mut()
    }

    /// Toggles the attached speed dial, if any.
    #[cfg(full_widgets)]
    fn toggle_speed_dial(&mut self) {
        if let Some(dial) = self.speed_dial.as_deref_mut() {
            dial.toggle();
        }
    }

    /// Advances the press and visibility animations; `true` while either is moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let pressed = self.press.tick(delta_ms);
        let visible = self.visibility.tick(delta_ms);
        #[cfg(full_widgets)]
        let dial = self.speed_dial.as_deref_mut().map(|dial| dial.tick(delta_ms)).unwrap_or(false);
        #[cfg(not(full_widgets))]
        let dial = false;
        pressed || visible || dial
    }

    /// Whether either animation is between two values.
    pub fn is_animating(&self) -> bool {
        let own = self.press.is_moving() || self.visibility.is_moving();
        #[cfg(full_widgets)]
        let dial = self.speed_dial.as_deref().is_some_and(|dial| dial.is_animating());
        #[cfg(not(full_widgets))]
        let dial = false;
        own || dial
    }

    /// Retargets the press driver from the control's own pressed latch.
    fn sync_press_driver(&mut self) {
        self.press.set_target(if self.base.is_pressed() { 1.0 } else { 0.0 });
    }

    /// Returns the button's shadow depth.
    pub fn elevation(&self) -> f32 {
        self.elevation
    }

    /// Sets the button's shadow depth, clamped to `0.0..=8.0` (OV-07).
    ///
    /// The default is `3.0`, which is the value every previous frame was drawn with, so a caller
    /// that never asks keeps the historical projection.
    pub fn set_elevation(&mut self, elevation: f32) {
        if !elevation.is_finite() {
            return;
        }
        let elevation = elevation.clamp(0.0, 8.0);
        if (self.elevation - elevation).abs() < f32::EPSILON {
            return;
        }
        self.elevation = elevation;
        self.base.request_redraw();
    }

    /// Programmatically trigger a click cycle (press then release).
    pub fn click(&mut self) {
        self.base.clicked.emit();
        #[cfg(full_widgets)]
        self.toggle_speed_dial();
    }
}

impl Widget for FAB {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Keeps an attached speed dial on the button's rectangle (OV-009).
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        #[cfg(full_widgets)]
        if let Some(dial) = self.speed_dial.as_deref_mut() {
            dial.set_geometry(geometry);
        }
    }

    fn size_hint(&self) -> crate::core::Size {
        let edge = self.size_kind.edge();
        let width = match self.extended_label.as_deref() {
            // Measure the label with the shared text ruler rather than charging eight pixels per
            // character (OV-004). The font is the one the capsule paints the label at (the
            // draw's `radius * 0.5` floors at 14), so the hint and the ink agree on a
            // proportional face instead of only on a monospace one.
            Some(label) => {
                let font = Font::new("sans-serif", 14.0, false, false);
                edge + 16 + crate::widget::metrics::estimate_text_width(label, &font, 1.0)
            }
            None => edge,
        };
        crate::core::Size::new(width, edge)
    }

    /// Advances the press and visibility animations.
    fn tick(&mut self, delta_ms: u32) -> bool {
        FAB::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        FAB::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "clicked" => Some(EventSignalRef::unit("clicked", self.clicked_signal())),
            _ => None,
        }
    }
}

/// `FAB`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FAB {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "icon" => Ok(CapabilityValue::String(self.icon_text().to_string())),
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                // A borrowed static-geometry source has no document representation, so reading it
                // back as one would be a lie; the caller is told instead.
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "icon" => {
                self.set_icon_text(&expect_string(value)?);
                Ok(())
            }
            "icon_source" => {
                match value {
                    CapabilityValue::Null => self.set_icon_source(None),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(Some(IconSource::Builtin(name))),
                    // A resource ID needs a host resolver; turning it into a path here would be
                    // guessing at the host's resource policy.
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn set_resolved_icon_source(
        &mut self,
        source: Option<IconSource>,
    ) -> Result<(), CapabilityAccessError> {
        self.set_icon_source(source);
        Ok(())
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["icon", "icon_source", BASE_PROPERTY_NAMES]
    }
}

impl Draw for FAB {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let dpi = self.base.dpi_scale();
        let is_enabled = self.base.is_enabled();

        // Compute button center and radius
        let cx = rect.x + (rect.width as i32) / 2;
        let cy = rect.y + (rect.height as i32) / 2;
        // The shadow is the same circle, offset down-right, so the *painted* extent is the
        // radius plus that offset on the bottom and right. A radius of `min(w, h) / 2` filled
        // the box exactly, which left the offset circle sticking out on two sides: the SVG
        // snapshot showed the shadow hanging past the control while the raster backend clipped
        // it. The radius is therefore reduced by the largest offset used (the unpressed one,
        // since the pressed state shrinks the circle anyway), keeping the shadow inside too.
        let max_shadow_offset = (self.elevation * dpi).ceil() as u32;
        let base_radius =
            (rect.width.min(rect.height) / 2).saturating_sub(max_shadow_offset / 2 + 1);

        // When pressed, the shrink follows the driver rather than the boolean: at rest the
        // fraction is exactly zero, so the radius is the historical one, and a press now
        // reaches that radius over frames instead of in one.
        let press = if is_enabled { self.press.value().clamp(0.0, 1.0) } else { 0.0 };
        let visibility = self.visibility.value().clamp(0.0, 1.0);
        if visibility <= 0.001 {
            return;
        }

        // The attached speed dial paints first so this button stays on top of its own primary
        // circle, which the dial would otherwise duplicate at the same centre (OV-009). A button
        // that has scrolled away does not leave its dial behind.
        #[cfg(full_widgets)]
        if let Some(dial) = self.speed_dial.as_deref_mut() {
            dial.draw(context);
        }
        let radius = (base_radius as f32 * (1.0 - 0.1 * press)) as u32;
        let shadow_offset = if press > 0.0 {
            ((self.elevation - press).max(0.0) * dpi) as u32
        } else {
            (self.elevation * dpi) as u32
        };
        // The hide-on-scroll transition moves the button below its own band as it leaves.
        let hide_shift = ((1.0 - visibility) * rect.height as f32) as i32;
        let cy = cy + hide_shift;
        let center = Point::new(cx, cy);

        // Draw shadow (offset darker circle underneath)
        let shadow_center =
            Point::new(cx + shadow_offset as i32 / 2, cy + shadow_offset as i32 / 2);
        let shadow_color = Color::rgba(0, 0, 0, 60);
        context.fill_circle_aa(shadow_center, radius, shadow_color);

        // Determine button fill color. The constructor's default is a literal, so a FAB that
        // has never been re-coloured by its caller reads the theme instead: explicit style
        // first, then the theme's resolved style for this control, and only then the literal.
        // Without that step a theme switch left the FAB the same blue in both appearances.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // Resolve the style for the control's *current* state, so a theme's
        // `fab:hover`/`fab:pressed`/`fab:focus` entries are reachable (OV-003). The normal state
        // resolves to the same resting style the control painted before.
        let state = self.widget_state();
        let theme = crate::style::resolved_theme_style_for_state_shared("fab", state);
        // `fab` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill, not an
        // action colour. The circle is a filled call to action, so the theme's `primary` is
        // read directly rather than the mis-classified role's surface.
        let themed_ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| self.accent_color.contrast_color());
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(self.accent_color);
        let caller_set_accent = self.accent_color != Self::DEFAULT_ACCENT_COLOR;
        let base_fill = if caller_set_accent {
            self.accent_color
        } else {
            style.background_color.unwrap_or(accent)
        };
        // A hover or press answers the pointer even when the theme has no state override. At rest
        // this is an identity, so the historical fill is byte-identical (OV-003).
        let interaction = match state {
            crate::style::WidgetState::Hover => 0.07,
            crate::style::WidgetState::Pressed => 0.14,
            _ => 0.0,
        };
        let base_fill =
            if interaction > 0.0 { base_fill.blend(&themed_ink, interaction) } else { base_fill };
        let fill_color = if !is_enabled {
            Color::rgba(base_fill.r, base_fill.g, base_fill.b, 120)
        } else {
            base_fill
        };
        // The icon is drawn on the fill, so it takes the same contrast decision the theme
        // uses for a filled action rather than a fixed white that can vanish on a light accent.
        let icon_color = if !is_enabled { themed_ink.with_alpha_f32(0.5) } else { themed_ink };

        // Draw filled circle, or an extended capsule when a label is set.
        if let Some(label) = self.extended_label.as_deref() {
            let capsule = Rect::new(
                rect.x + max_shadow_offset as i32,
                cy - radius as i32,
                (rect.width.saturating_sub(max_shadow_offset * 2)).max(1),
                radius * 2,
            );
            context.fill_rounded_rect(capsule, radius, fill_color);
            let font_size = (radius as f32 * 0.5).max(14.0);
            let bits = font_size.to_bits();
            if self.label_font_cache.as_ref().is_none_or(|(key, _)| *key != bits) {
                self.label_font_cache =
                    Some((bits, Font::new("sans-serif", font_size, false, false)));
            }
            let font =
                self.label_font_cache.as_ref().map(|(_, font)| font.clone()).unwrap_or_default();
            let label_band = Rect::new(
                capsule.x + radius as i32,
                context.text_line(capsule, &font).y,
                capsule.width.saturating_sub(radius),
                radius * 2,
            );
            context.draw_text_fitted_cow(
                label_band,
                label,
                &font,
                icon_color,
                HorizontalAlignment::Left,
            );
        } else {
            context.fill_circle_aa(center, radius, fill_color);
        }

        // Draw icon text centred on the button. The glyph origin is the glyph's **top-left**,
        // so the vertical centre is half the line box; adding `ascent` on top of an
        // already-centred y pushed the icon half a line below the circle's middle. The label
        // is fitted to the circle's inner square so a long one is truncated rather than
        // painted out of the button.
        if let Some(source) = self.icon_source {
            let half_side = (radius as f32 / core::f32::consts::SQRT_2) as i32;
            let side = (half_side * 2).max(0) as u32;
            let icon_rect = Rect::new(cx - half_side, cy - half_side, side, side);
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, icon_color)
            {
                log::warn!("floating action button icon could not be painted: {error:?}");
            }
        } else if !self.icon_text.is_empty() {
            let font_size = if self.size_kind == FabSize::Mini {
                (radius as f32 * 0.7).max(12.0)
            } else {
                (radius as f32 * 0.7).max(16.0)
            };
            let bits = font_size.to_bits();
            if self.icon_font_cache.as_ref().is_none_or(|(key, _)| *key != bits) {
                self.icon_font_cache =
                    Some((bits, Font::new("sans-serif", font_size, false, false)));
            }
            let font =
                self.icon_font_cache.as_ref().map(|(_, font)| font.clone()).unwrap_or_default();

            let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                &self.icon_text,
                &font,
            );
            let text_y = cy - (metrics.height as i32) / 2;
            // The inscribed square of a circle of radius `r` has a half-side of `r / sqrt(2)`,
            // which is the widest label that stays inside the disc at any rotation.
            let half_side = (radius as f32 / core::f32::consts::SQRT_2) as i32;
            let band = Rect::new(
                (cx - half_side).max(rect.x),
                text_y.max(rect.y),
                (half_side * 2).max(0) as u32,
                metrics.height,
            );
            context.draw_text_fitted_cow(
                band,
                &self.icon_text,
                &font,
                icon_color,
                HorizontalAlignment::Center,
            );
        }

        // Keyboard focus is the shared inset ring, gated on the reason focus arrived. A circle
        // gets a circular ring rather than a rounded box, which is what "inset" means for this
        // shape (OV-003).
        if self.base.draws_focus_ring() {
            let ring = crate::widget::metrics::FocusRing::for_control(rect, radius);
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    crate::widget::metrics::focus_ring_color(icon_color),
                    crate::widget::metrics::FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

impl EventHandler for FAB {
    /// The same activation contract `Button` implements.
    ///
    /// # What was wrong
    ///
    /// Both pointer arms discarded the position (`pos: _`), so **any** release the host
    /// routed here committed a click — including a drag that began outside the FAB and
    /// merely ended on top of it, and any release delivered without a preceding press
    /// inside. There was also no `MouseLeave` arm, so the `pressed` latch survived the
    /// pointer leaving and stayed armed for an unrelated later release. `Button` had
    /// already been fixed; this is the same control in a different shape, and leaving it
    /// as the one outlier is exactly the "fixed one, missed the family" shape
    /// `tools/check_click_requires_release_inside.sh` exists to catch.
    ///
    /// A release outside **cancels** and does not emit `clicked`; a release inside
    /// emits it. `MouseLeave` clears the latch without emitting, because a pointer that
    /// merely crosses the edge may still come back.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == crate::event::mouse_button::PRIMARY {
                    // Only a press that lands on the FAB arms it. The runtime hit-tests
                    // before delivery, but a direct dispatch does not, and an unguarded
                    // press leaves the latch armed for a release belonging elsewhere.
                    self.base.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
                    self.sync_press_driver();
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos, button } => {
                if *button == crate::event::mouse_button::PRIMARY && self.base.is_pressed() {
                    self.base.set_pressed(false);
                    self.sync_press_driver();
                    // Inside commits; outside cancels silently. `clicked` is what a caller
                    // routes an action on, so it must mean "this gesture completed here".
                    if self.base.contains_point_with_touch_expansion(*pos) {
                        self.base.clicked.emit();
                        #[cfg(full_widgets)]
                        self.toggle_speed_dial();
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                // Crossing the edge abandons the press. Only the latch is cleared — no
                // signal — because the release that ends the gesture decides its outcome.
                if self.base.is_pressed() {
                    self.base.set_pressed(false);
                    self.sync_press_driver();
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.base.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
                self.sync_press_driver();
                self.base.request_redraw();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } => {
                if self.base.is_pressed() {
                    self.base.set_pressed(false);
                    self.sync_press_driver();
                    if self.base.contains_point_with_touch_expansion(*pos) {
                        self.base.clicked.emit();
                        #[cfg(full_widgets)]
                        self.toggle_speed_dial();
                    }
                    self.base.request_redraw();
                }
            }
            // A `Tap` carries no position: the platform already resolved it to this control,
            // which is the same basis the hit test narrows.
            #[cfg(feature = "touch")]
            Event::Tap { .. } => {
                self.base.clicked.emit();
                #[cfg(full_widgets)]
                self.toggle_speed_dial();
                self.base.request_redraw();
            }
            Event::Wheel { delta, .. } => {
                self.report_scroll(delta.y);
                self.base.handle_event(event);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_fab() -> FAB {
        FAB::new(Rect::new(0, 0, 56, 56))
    }

    /// `FAB`'s own `icon_source` contract, alongside its legacy `icon` text property.
    #[test]
    fn fab_icon_source_property_round_trips_and_refuses_the_unresolvable() {
        use crate::widget::capability::types::CapabilityValue as V;
        use crate::widget::icon_source::{IconColorMode, IconSpec};

        let mut fab = make_fab();
        assert_eq!(fab.get("icon_source"), Ok(V::Null));
        assert_eq!(fab.get("icon"), Ok(V::String("+".to_string())));

        let icon = IconSpec::Builtin(crate::widget::IconName::Edit);
        fab.set("icon_source", V::IconSpec(icon.clone()))
            .expect("built-in descriptors apply without a resolver");
        assert_eq!(fab.get("icon_source"), Ok(V::IconSpec(icon)));
        // The typed setter owns the one active state, so the legacy text no longer reports.
        assert_eq!(fab.get("icon"), Ok(V::String(String::new())));

        assert_eq!(
            fab.set(
                "icon_source",
                V::IconSpec(IconSpec::Resource {
                    id: "project/icons/plus".into(),
                    color_mode: IconColorMode::Original,
                }),
            ),
            Err(CapabilityAccessError::ResourceResolutionRequired)
        );

        // The legacy text property still works, and it replaces the typed source in turn.
        fab.set("icon", V::String("✕".to_string())).expect("the legacy text property is writable");
        assert_eq!(fab.icon_source(), None);
        assert_eq!(fab.get("icon"), Ok(V::String("✕".to_string())));

        assert_eq!(
            fab.set_resolved_icon_source(Some(IconSource::Builtin(crate::widget::IconName::Plus))),
            Ok(())
        );
        assert_eq!(fab.icon_source(), Some(IconSource::Builtin(crate::widget::IconName::Plus)));
    }

    #[test]
    fn fab_default_creation() {
        let fab = make_fab();
        assert_eq!(fab.kind(), WidgetKind::FAB);
        assert_eq!(fab.icon_text(), "+");
        assert_eq!(fab.accent_color(), Color::PRIMARY);
        assert!(!fab.is_mini());
        assert_eq!(fab.geometry(), Rect::new(0, 0, 56, 56));
        assert!(fab.is_visible());
        assert!(fab.is_enabled());
    }

    #[test]
    fn fab_icon_text() {
        let mut fab = make_fab();
        assert_eq!(fab.icon_text(), "+");

        fab.set_icon_text("✕");
        assert_eq!(fab.icon_text(), "✕");

        fab.set_icon_text("↓");
        assert_eq!(fab.icon_text(), "↓");
    }

    #[test]
    fn fab_typed_source_replaces_text_and_reaches_renderer() {
        let mut fab = make_fab();
        let source = IconSource::builtin(crate::widget::IconName::Plus);
        fab.set_icon_source(Some(source));
        assert_eq!(fab.icon_source(), Some(source));
        assert!(fab.icon_text().is_empty());
        let svg = render_to_svg(&mut fab);
        assert!(svg.contains("<path"), "typed geometry must be painted in the FAB");

        fab.set_icon_text("+");
        assert_eq!(fab.icon_source(), None);
        assert_eq!(fab.icon_text(), "+");
    }

    #[test]
    fn fab_accent_color() {
        let mut fab = make_fab();
        assert_eq!(fab.accent_color(), Color::PRIMARY);

        fab.set_accent_color(Color::ERROR);
        assert_eq!(fab.accent_color(), Color::ERROR);

        fab.set_accent_color(Color::SUCCESS);
        assert_eq!(fab.accent_color(), Color::SUCCESS);
    }

    #[test]
    fn fab_mini_mode() {
        let mut fab = make_fab();
        assert!(!fab.is_mini());

        fab.set_mini(true);
        assert!(fab.is_mini());

        fab.set_mini(false);
        assert!(!fab.is_mini());
    }

    #[test]
    fn fab_click_signal_emits() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.click();
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_mouse_press_release_emits_clicked() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1, modifiers: 0 });
        assert!(fab.base.is_pressed());
        assert!(!clicked.load(Ordering::SeqCst));

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!fab.base.is_pressed());
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_mouse_press_release_other_button_noop() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 2, modifiers: 0 });
        assert!(!fab.base.is_pressed());
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_disabled_blocks_events() {
        let mut fab = make_fab();
        fab.set_enabled(false);

        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1, modifiers: 0 });
        assert!(!fab.base.is_pressed());

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn fab_tap_emits_clicked() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::Tap { pos: Point::new(28, 28) });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_svg_output() {
        let mut fab = make_fab();
        let svg = render_to_svg(&mut fab);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"56\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn fab_svg_output_mini() {
        let mut fab = FAB::new(Rect::new(0, 0, 40, 40));
        fab.set_mini(true);
        let svg = render_to_svg(&mut fab);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"40\""));
        assert!(svg.contains("height=\"40\""));
    }

    #[test]
    fn fab_press_animation_sets_pressed_state() {
        let mut fab = make_fab();
        assert!(!fab.base.is_pressed());

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1, modifiers: 0 });
        assert!(fab.base.is_pressed());

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!fab.base.is_pressed());
    }

    #[test]
    fn fab_clicked_signal_accessor() {
        let fab = make_fab();
        let signal = fab.clicked_signal();
        // Signal should be valid and connectable
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn the_press_shrink_is_animated_rather_than_switched() {
        let mut fab = make_fab();
        fab.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        assert!(fab.is_animating(), "a press starts a movement");
        assert!(fab.tick(16));
        fab.handle_event(&Event::MouseRelease { pos: Point::new(5, 5), button: 1 });
        while fab.tick(16) {}
        assert!(!fab.is_animating());
    }

    #[test]
    fn an_extended_fab_reports_a_capsule_width() {
        let mut fab = FAB::new(Rect::new(0, 0, 56, 56));
        let round = fab.size_hint();
        fab.set_extended_label(Some("Compose"));
        assert!(fab.size_hint().width > round.width);
        assert_eq!(fab.extended_label(), Some("Compose"));
        fab.set_extended_label(None::<String>);
        assert_eq!(fab.size_hint().width, round.width);
    }

    /// The capsule width comes from the text ruler, not from a per-character constant (OV-004).
    #[test]
    fn an_extended_label_is_measured_rather_than_estimated_at_eight_pixels_per_character() {
        let mut fab = FAB::new(Rect::new(0, 0, 56, 56));
        fab.set_extended_label(Some("iii"));
        let narrow = fab.size_hint().width;
        fab.set_extended_label(Some("WWW"));
        let wide = fab.size_hint().width;
        assert!(
            wide > narrow,
            "a proportional face gives wide glyphs more room than narrow ones: {narrow} vs {wide}"
        );
        assert_eq!(
            wide,
            56 + 16
                + crate::widget::metrics::estimate_text_width(
                    "WWW",
                    &Font::new("sans-serif", 14.0, false, false),
                    1.0,
                ),
            "the hint reports the measured advance"
        );
    }

    /// Hover is a visible state on the button, not only a theme-table lookup (OV-003).
    #[test]
    fn a_hovered_fab_paints_differently_from_a_resting_one() {
        let mut fab = make_fab();
        let resting = render_to_svg(&mut fab);
        fab.handle_event(&Event::MouseEnter { pos: Point::new(28, 28) });
        let hovered = render_to_svg(&mut fab);
        assert_ne!(resting, hovered, "the button must answer the pointer");
    }

    /// A FAB can drive its companion speed dial without either owning the other's state (OV-009).
    #[cfg(full_widgets)]
    #[test]
    fn a_fab_toggles_an_attached_speed_dial() {
        use crate::widget::companions::SpeedDial;

        let mut fab = make_fab();
        assert!(fab.speed_dial().is_none(), "the slot is opt-in");
        fab.set_speed_dial(Some(SpeedDial::new(Rect::new(0, 0, 56, 56))));
        assert!(fab.speed_dial().is_some());

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1, modifiers: 0 });
        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(
            fab.speed_dial().is_some_and(|dial| dial.is_expanded()),
            "a completed click opens the dial"
        );

        fab.click();
        assert!(
            fab.speed_dial().is_some_and(|dial| !dial.is_expanded()),
            "the next click closes it again"
        );
    }

    #[test]
    fn scrolling_hides_and_reshows_the_fab_only_when_asked() {
        let mut fab = make_fab();
        fab.report_scroll(3);
        assert_eq!(fab.visibility_progress(), 1.0, "hiding is opt-in");
        fab.set_hide_on_scroll(true);
        fab.report_scroll(3);
        while fab.tick(16) {}
        assert_eq!(fab.visibility_progress(), 0.0);
        fab.report_scroll(-3);
        while fab.tick(16) {}
        assert_eq!(fab.visibility_progress(), 1.0);
    }

    #[test]
    fn elevation_is_configurable_and_clamped() {
        let mut fab = make_fab();
        assert_eq!(fab.elevation(), 3.0, "the historical depth is the default");
        fab.set_elevation(6.0);
        assert_eq!(fab.elevation(), 6.0);
        fab.set_elevation(20.0);
        assert_eq!(fab.elevation(), 8.0, "the upper bound keeps the shadow inside the box");
        fab.set_elevation(-4.0);
        assert_eq!(fab.elevation(), 0.0, "zero removes the shadow rather than inverting it");
        fab.set_elevation(f32::NAN);
        assert_eq!(fab.elevation(), 0.0, "a non-finite write is ignored");
    }
}
