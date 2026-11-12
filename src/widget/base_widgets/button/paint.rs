use super::*;

impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering with style integration (BLUE13 R1.3)
        // Reads style fields (background_color, text_color, border_color, etc.)
        // falling back to state-based defaults when style values are not set.
        let state = self.state();
        // The icon is composed **before** the style borrow below. Tinting mutates this control's
        // composed-copy cache, and the style is an immutable borrow of the whole control that lasts
        // until its last field read — so a `&mut self` call inside that span cannot compile, and a
        // per-frame style clone to work around it would cost more than the tint.
        let icon_box = self.icon_rect();
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        let tinted_shared_icon = self
            .shared_icon_pixels
            .clone()
            .and_then(|pixels| self.icon_tint.compose_pixels(&pixels));
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        let tinted_legacy_icon = match (self.icon.as_ref(), icon_box) {
            (Some(image), Some(box_)) => {
                self.icon_tint.compose_image(image, (box_.width, box_.height))
            }
            _ => None,
        };
        let style = self.style();

        // ── The box actually painted ──
        //
        // A button fills the width it was *given* — that is what a button is, and a wider
        // button is still a button. But its **height** is its own: a 240x120 census cell drew a
        // 240x120 pill, which is a rectangle shaped like a button rather than a button. The
        // height comes from `size_hint`, the same derivation a layout asks for, so the drawn
        // shape and the reported size cannot disagree.
        let rect = self.painted_rect();

        // ── Background ──
        let bg = style.background_color.unwrap_or_else(|| match state {
            ButtonState::Normal => Color::rgb(240, 240, 240),
            ButtonState::Pressed => Color::rgb(200, 200, 200),
            ButtonState::Disabled => Color::rgb(220, 220, 220),
        });
        // The fill blends from the resting colour toward the interactive one by the transition's own
        // progress, so a hover fades in and a press deepens it rather than both snapping. The
        // resting and pressed colours are the ones this control already used; only the *path*
        // between them is new.
        //
        // At progress `0.0` — a fresh control, or one whose transition has settled at rest — the
        // result is exactly the resting colour, so a snapshot taken without ticking is unchanged.
        let progress = self.interaction_progress.value();
        let bg = if progress <= 0.0 {
            bg
        } else {
            let interactive = match state {
                ButtonState::Pressed => bg,
                _ => bg.blend(&bg.contrast_color(), crate::style::HOVER_FILL_BLEND),
            };
            bg.blend(&interactive, progress)
        };
        // The corner radius is a profile shape token, not a second colour fact. A radius the
        // caller actually chose is honoured even when it equals the Material baseline; the
        // baseline itself is the theme's generic radius rather than a button-specific choice,
        // so a theme-authored value follows the active profile instead — 6 px on macOS,
        // Material's own 4 everywhere else. An unset radius keeps drawing square, which is what
        // a control with no theme has always done.
        let profile = profile_metrics(active_profile());
        let br = match style.border_radius {
            Some(radius) if self.border_radius_explicit => radius,
            Some(_) => profile.button_radius,
            None => 0,
        };
        // ── The face ──
        //
        // The fill, the bevel and the outline are painted in one call, because they are one
        // operation with an order that matters (fill, then bevel on the boundary, then the
        // outermost line). The surface comes from the style — the role table's default for a
        // button, as any theme override replaced it — so a theme that says "raise this face" or
        // "flatten it" changes what a button is drawn as **without a line of code here**
        // (BLUE24 §10A).
        //
        // `solid()` is the identity, so a button on the flat preset emits exactly the fill and
        // outline it emitted before surfaces existed: the snapshot suite is the proof.
        let surface = style.surface.unwrap_or_else(crate::render::SurfaceStyle::solid);
        if self.flat {
            // A flat button shows no face of its own. A background the **caller** declared is still
            // honoured (that is an explicit request), and the interaction wash is painted so the
            // control acknowledges the pointer — but the theme's own role fill is chrome, which is
            // exactly what `flat` removes.
            let caller_background = !style.theme_derived
                && (style.background_color.is_some() || style.background_gradient.is_some());
            if caller_background {
                context.face_with_gradient(
                    rect,
                    bg,
                    style.background_gradient.as_ref(),
                    surface,
                    br,
                    Color::BLACK,
                );
            } else if progress > 0.0 {
                let interactive = bg.blend(&bg.contrast_color(), crate::style::HOVER_FILL_BLEND);
                context.fill_rounded_rect(rect, br, interactive);
            }
            if let (Some(color), Some(width)) = (style.border_color, style.border_width) {
                if width > 0 {
                    context.draw_rounded_rect_stroke(rect, br, color, width);
                }
            }
        } else {
            surface.paint_with_gradient(
                context,
                rect,
                bg,
                style.background_gradient.as_ref(),
                style.border_color.unwrap_or(bg),
                style.border_width.unwrap_or(0),
                br,
                Color::BLACK,
            );
        }

        // ── Icon ──
        //
        // The icon is the **leading** content slot, so its box comes from `icon_rect` — the same
        // derivation `implicit_size` reserves room with — rather than from a second, hand-written
        // offset. §B.9's point is exactly this: a wider icon pushes the label along instead of
        // overlapping it.
        let icon_color = self.icon_tint.color_or(style.text_color.unwrap_or_else(|| {
            let fill = style.background_color.unwrap_or(Color::WHITE);
            if state == ButtonState::Disabled {
                fill.contrast_color().with_alpha(150)
            } else {
                fill.contrast_color()
            }
        }));
        if let (Some(source), Some(box_)) = (self.icon_source, icon_box) {
            if let Err(error) = crate::widget::paint_icon(context, source.into(), box_, icon_color)
            {
                log::warn!("button icon could not be painted: {error:?}");
            }
        }

        #[cfg(feature = "image")]
        if self.icon_source.is_none() {
            #[cfg(not(alloc_frugal))]
            if let (Some(box_), Some(pixels)) =
                (icon_box, tinted_shared_icon.as_ref().or(self.shared_icon_pixels.as_ref()))
            {
                context.draw_image_scaled(
                    box_.x,
                    box_.y,
                    box_.width,
                    box_.height,
                    pixels.width(),
                    pixels.height(),
                    pixels.pixels(),
                );
            } else if let (Some(icon), Some(box_)) = (self.icon.as_ref(), icon_box) {
                // The icon is scaled to the slot the metric derivation reserved for it, so the room
                // `implicit_size` added and the pixels drawn are the same square. An icon whose own
                // bitmap is a different size is scaled rather than drawn at its native extent —
                // otherwise the button's reserved advance and its ink would disagree.
                //
                // With a tint the copy is drawn into the **same** box, so a tint changes the
                // colours and nothing else about the layout.
                match tinted_legacy_icon.as_ref() {
                    // The composed handle is already at the slot size.
                    Some(pixels) => {
                        context.draw_image(box_.x, box_.y, box_.width, box_.height, pixels.pixels())
                    }
                    None => context.draw_image_scaled(
                        box_.x,
                        box_.y,
                        box_.width,
                        box_.height,
                        icon.width(),
                        icon.height(),
                        icon.rgba8_data().unwrap_or(&[]),
                    ),
                }
            }
            #[cfg(alloc_frugal)]
            if let (Some(icon), Some(box_)) = (self.icon.as_ref(), icon_box) {
                // The icon is scaled to the slot the metric derivation reserved for it, so the room
                // `implicit_size` added and the pixels drawn are the same square. An icon whose own
                // bitmap is a different size is resampled rather than drawn at its native extent —
                // otherwise the button's reserved advance and its ink would disagree.
                context.draw_image_scaled(
                    box_.x,
                    box_.y,
                    box_.width,
                    box_.height,
                    icon.width(),
                    icon.height(),
                    icon.rgba8_data().unwrap_or(&[]),
                );
            }
        }

        // ── Text ──
        if !self.text.is_empty() {
            // The draw path and `implicit_size` take their font from the same accessor, so the
            // label measured and the label painted cannot disagree (D09-STYLE-02).
            let font = effective_font(style);
            // The ink must be legible *on the fill this button actually got*, not on an
            // assumption about it. The fill comes from `style.background_color`, which the
            // theme may have set to a saturated accent: the dark appearance's `PRIMARY` is
            // `rgb(100,181,246)`, and the old rule drew pure black on it — chosen from
            // `state == Disabled` alone, with no reference to the background at all, so
            // `button.svg` carried a black label on a light-blue pill. Deriving the ink from
            // the resolved fill is what makes "one foreground per background" true for any
            // theme, which is the same rule `badge` and `role_colors` already use.
            let fill = style.background_color.unwrap_or(Color::WHITE);
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ButtonState::Disabled {
                    // A disabled button's ink is *deliberately* low-contrast: that is what
                    // makes it read as unavailable. It is still derived from the fill's own
                    // luminance so it recedes in the dark appearance too instead of
                    // disappearing into it.
                    fill.contrast_color().with_alpha(150)
                } else {
                    fill.contrast_color()
                }
            });
            // Vertically centred through the shared primitive, so the label sits in the
            // button's middle instead of having its glyph-box top edge on that middle line.
            // A stacked icon gives the label its own band below or above the icon; an inline
            // icon leaves the label centred in the whole button, exactly as before.
            let label_box = self.label_rect();
            let line = match self.icon_position {
                IconPosition::Top | IconPosition::Bottom => context.text_line(label_box, font),
                IconPosition::Leading | IconPosition::Trailing => context.text_line(rect, font),
            };
            // Horizontally centred in the button's own box, which is what a push button's
            // label is: a push button's `contentItem` is an `AbstractButton`
            // whose `Text` is horizontally centred, and Material M3's
            // `TextButton`/`ElevatedButton` centre their child the same way. The label used to
            // start at `rect.x + BUTTON_PADDING_H`, i.e. flush to the left inside a pill many
            // times its own width — the label read as a left-aligned caption rather than as
            // the button's own name, and `snapshots/svg/button.svg` showed it at x = 12 in a
            // 240-wide control while `toggle_button` and `tool_button` centred theirs.
            //
            // `BUTTON_PADDING_H` is still the right constant here, but as the **minimum**
            // inset from either edge rather than as a left anchor: it is what keeps a label
            // that nearly fills the button from touching the border, and it is the same
            // number `size_hint` adds to the content when deriving the intrinsic width, so the
            // drawn label and the reported size cannot disagree. Centring inside the padded
            // box, rather than inside the raw rectangle, is what gives a wide button the room
            // the constant promises.
            // The label's own box comes from `label_rect`, which is the content box with the
            // icon's slot removed. That is the other slot of the same two-part composition, so a
            // button with an icon centres its label in the room the icon left rather than under
            // it. Without an icon the slot is zero and this is the whole content box.
            let label_bounds = Rect::new(label_box.x, line.y, label_box.width, line.height);
            match self.elide_mode {
                // `ElideRight` is the historical fitted draw, byte for byte.
                TextElideMode::ElideRight => {
                    context.draw_text_fitted_ellipsis(
                        label_bounds,
                        &self.text,
                        font,
                        text_color,
                        HorizontalAlignment::Center,
                    );
                }
                // `None` paints the whole string at the aligned origin, which is the "let the
                // renderer clip" behaviour a caller asks for explicitly.
                TextElideMode::None => {
                    let width = context.measure_text(&self.text, font).width as i32;
                    let x = label_bounds.x + (label_bounds.width as i32 - width).max(0) / 2;
                    context.draw_text(
                        Point::new(x, label_bounds.y),
                        &self.text,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }
                mode => draw_elided_line(
                    context,
                    label_bounds,
                    &self.text,
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                    mode,
                ),
            }
        }

        // ── Shortcut hint ──
        //
        // A declared shortcut is drawn as a small label in the bottom-right corner, which is
        // where every desktop toolkit places it and where it cannot be confused with the
        // button's own centred caption. `None` paints nothing, so the historical picture is
        // unchanged.
        if let Some(shortcut) = self.shortcut.as_ref() {
            // The formatted caption and its font come from the cache keyed by the same inputs as
            // the intrinsic-size hint (WGT-F7). The hit path compares the key and then draws from
            // borrowed cache contents; it used to clone the whole `Option<(key, String, Font)>`
            // before comparing, so every button with a shortcut allocated a `String` per frame
            // (WG-07).
            let base_font = effective_font(style);
            let key = self.hint_key(base_font);
            let hit = {
                let cache = self.shortcut_hint_cache.borrow();
                matches!(cache.as_ref(), Some((cached_key, _, _)) if cached_key == &key)
            };
            if !hit {
                let text = shortcut.format_shortcut();
                let font = base_font.with_size((base_font.size() * 0.75).max(6.0));
                *self.shortcut_hint_cache.borrow_mut() = Some((key, text, font));
            }
            let cache = self.shortcut_hint_cache.borrow();
            if let Some((_, hint, hint_font)) = cache.as_ref() {
                let pad = dimensions::BUTTON_PADDING_H as i32 / 2;
                let line = context.text_line(rect, hint_font);
                let band = Rect::new(
                    rect.x + pad,
                    rect.y + rect.height as i32 - line.height as i32 - pad,
                    rect.width.saturating_sub((pad as u32).saturating_mul(2)),
                    line.height,
                );
                let hint_line = context.text_line(band, hint_font);
                let ink = style.text_color.unwrap_or_else(|| bg.contrast_color()).with_alpha(170);
                context.draw_text(
                    Point::new(band.x + band.width as i32, hint_line.y),
                    hint,
                    hint_font,
                    ink,
                    HorizontalAlignment::Right,
                );
            }
        }

        // ── Default-button emphasis ──
        //
        // A default button is the one Enter activates, and every desktop convention says so *on
        // screen* -- a heavier outline, or the accent. The flag was stored, published and toggled by
        // the schema while `draw` never read it, so `set_default(true)` redrew an identical button
        // and the user had no way to see which one Enter would press.
        //
        // It is drawn **between** the border and the focus ring, and inside the control's own
        // rectangle, for the same reason the focus ring is: a ring outside the rect is the
        // "drawing leaves the control and only the raster clip hides it" defect the census catches.
        // The colour is the theme's accent: the emphasis has to be visible on a button whose
        // surface and border the host may have styled arbitrarily, and a contrast colour can be
        // black on the default light face, which is less of an emphasis than a hairline border.
        // A build with no palette falls back to that contrast colour, the only ink the resolved
        // face can guarantee.
        if self.is_default() {
            let ring = FocusRing::for_control(rect, br);
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    self.emphasis_ring_color(bg),
                    DEFAULT_EMPHASIS_WIDTH,
                );
            }
        }

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. This is the standard rule
        // for visual focus, and it is why `Button` carries a `FocusReason` rather than
        // a bare `focused` bool.
        //
        // The macOS metrics profile asks for the platform's outer accent halo instead of the inset
        // stroke. Both are selected here so the two styles cannot be mixed up: every other
        // profile keeps the inset ring byte for byte, which is what preserves the Material
        // snapshot.
        if self.visual_focus() {
            if active_profile() == ControlMetricsProfile::Macos {
                let halo = FocusRing::halo_for_control(rect, br);
                if halo.is_drawable() {
                    context.draw_box_shadow(
                        halo.rect,
                        focus_ring_color(bg.contrast_color()),
                        0,
                        0,
                        FOCUS_HALO_BLUR_RADIUS,
                        1,
                    );
                }
            } else {
                let ring = FocusRing::for_control(rect, br);
                if ring.is_drawable() {
                    context.draw_rounded_rect_stroke(
                        ring.rect,
                        ring.radius,
                        focus_ring_color(bg.contrast_color()),
                        FOCUS_RING_WIDTH,
                    );
                }
            }
        }

        // ── Drop feedback ──
        //
        // Only reachable when the caller opted in to drops; a plain button is unchanged.
        if self.drop_hovered {
            context.draw_rounded_rect_stroke(rect, br, focus_ring_color(bg.contrast_color()), 2);
        }

        // ── Attached menu ──
        //
        // Painted after the button's own chrome because it sits outside its rectangle. The
        // geometry comes from `menu_rect`, the same derivation the hit test uses, so a row is
        // clickable exactly where it was drawn.
        #[cfg(full_widgets)]
        if self.menu_open {
            let panel = self.menu_rect();
            let accent = crate::style::palette_shared()
                .as_ref()
                .map(|theme| theme.accent)
                .unwrap_or_else(|| bg.contrast_color());
            draw_menu_panel(context, panel, &self.menu_items, &[], style, accent);
            // The keyboard highlight is the button's own chrome, so it is painted here rather
            // than being a property of the shared menu painter (which the menu button owns).
            if let Some(index) = self.active_menu_index {
                if self.menu_items.get(index).is_some_and(|item| item.enabled) {
                    let row = menu_item_rect(panel, index);
                    context.draw_rounded_rect_stroke(row, 2, accent, 1);
                }
            }
        }
    }
}
