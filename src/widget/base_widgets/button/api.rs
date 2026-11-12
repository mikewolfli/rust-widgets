use super::*;

impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            #[cfg(feature = "image")]
            icon: None,
            icon_source: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            shared_icon_pixels: None,
            icon_tint: IconTint::new(),
            default_button: false,
            icon_size: dimensions::BUTTON_ICON_SIZE,
            icon_position: IconPosition::default(),
            shortcut: None,
            auto_default: false,
            auto_exclusive: false,
            flat: false,
            border_radius_explicit: false,
            elide_mode: TextElideMode::ElideRight,
            auto_repeat: AutoRepeat::new(),
            text_revision: 0,
            hint_cache: RefCell::new(None),
            shortcut_hint_cache: RefCell::new(None),
            #[cfg(full_widgets)]
            menu_items: Vec::new(),
            #[cfg(full_widgets)]
            menu_open: false,
            #[cfg(full_widgets)]
            menu_anchor: None,
            #[cfg(full_widgets)]
            active_menu_index: None,
            #[cfg(full_widgets)]
            popup_delay_ms: 300,
            #[cfg(full_widgets)]
            press_elapsed: None,
            drag_enabled: false,
            accepts_drops: false,
            drop_hovered: false,
            #[cfg(full_widgets)]
            item_triggered: crate::signal::Signal1::new(),
            signaled_pressed: false,
            // A freshly constructed button is at rest, so its progress is at the rest end of the
            // interpolation. Starting at the interactive end would make every button fade *out* on
            // its first frame. `Normal` is the tempo a state change takes.
            //
            // `PropertyDriver`'s own default is the same value, so this is the documented
            // rest state rather than a second, independent choice.
            interaction_progress: crate::style::PropertyDriver::default(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
            context_menu_requested: crate::signal::Signal1::new(),
            canceled: GenericSignal::new(),
            #[cfg(full_widgets)]
            action_id: None,
            #[cfg(full_widgets)]
            live_action: None,
        }
    }

    /// Links this button to a command in the action registry.
    ///
    /// The text and enabled state are read from the definition, the host binding is recorded, and
    /// a live cell subscribes to the command's signals. This is the button half of COMP-25; the
    /// click itself still emits `Button::clicked`, which a host routes to
    /// `ActionManager::trigger_action` (or to [`Self::trigger_linked_action`]).
    #[cfg(full_widgets)]
    pub fn bind_action(
        &mut self,
        manager: &mut crate::action::ActionManager,
        action_id: &str,
    ) -> bool {
        let Some(definition) = manager.action_definition(action_id) else {
            return false;
        };
        if !manager.bind_action_to_button(definition.id.clone(), self.base.id()) {
            return false;
        }
        self.set_text(definition.text);
        self.set_enabled(definition.enabled);
        self.live_action = manager.watch_action(&definition.id);
        self.action_id = Some(definition.id);
        self.base.request_redraw();
        true
    }

    /// The registry command this button presents, if any.
    #[cfg(full_widgets)]
    pub fn action_id(&self) -> Option<&str> {
        self.action_id.as_deref()
    }

    /// Applies the linked command's live state to this button.
    ///
    /// Called explicitly because a signal callback cannot borrow the widget; a host calls it from
    /// its event or paint path. Returns whether the button changed.
    #[cfg(full_widgets)]
    pub fn refresh_action_state(&mut self, manager: &crate::action::ActionManager) -> bool {
        let Some(action_id) = self.action_id.clone() else {
            return false;
        };
        let live = self.live_action.as_ref().map(|state| lock(state).clone());
        let definition = manager.action_definition(&action_id);
        let (text, enabled) = match (live, definition) {
            (Some(state), _) => (state.text, state.enabled),
            (None, Some(definition)) => (definition.text, definition.enabled),
            (None, None) => return false,
        };
        let mut changed = false;
        if self.text != text {
            self.set_text(text);
            changed = true;
        }
        if self.base.is_enabled() != enabled {
            self.set_enabled(enabled);
            changed = true;
        }
        changed
    }

    /// Triggers the linked command through the registry, reporting whether one fired.
    #[cfg(full_widgets)]
    pub fn trigger_linked_action(&self, manager: &mut crate::action::ActionManager) -> bool {
        self.action_id.as_deref().is_some_and(|id| manager.trigger_action(id))
    }
    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns whether this button currently owns keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// Returns whether a focus ring should be painted right now.
    ///
    /// The single question every draw site asks — [`BaseWidget::draws_focus_ring`] —
    /// evaluated in one place so a control cannot accidentally implement "has focus"
    /// as "draw the ring".
    pub fn visual_focus(&self) -> bool {
        self.base.draws_focus_ring()
    }

    /// Returns whether this button is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// The size this button claims when nothing constrains it — §B.7's `button_with_icon`
    /// template.
    ///
    /// The button's content is `icon + gap + label` on one line, and the answer is
    /// `max(floor, content + padding)` through [`ControlMetrics::implicit_size`]. The active
    /// profile's button floor is the load-bearing term — Material's `64x40` or macOS's shorter
    /// `64x28` — so a button labelled with two characters is still big enough to press.
    ///
    /// A profile with a compact floor also owns the vertical air: the height is the floor, and
    /// the space left around the content is split above and below it. Material's floor is taller
    /// than its content plus its named padding, so there its padding token remains the answer.
    ///
    /// # The two defects this replaces
    ///
    /// The old hint measured the label as `self.text().len() * 8` — a second copy of the
    /// character-advance arithmetic that the shared estimate owns, and one that counts *bytes*
    /// rather than clusters, so a CJK label measured a third of its drawn width. The icon's box
    /// was added to that same hand-rolled sum. Both now come from the metric system, which is what
    /// `check_implicit_size_uses_metrics` guards.
    pub fn implicit_size(&self) -> Size {
        let font = effective_font(self.style());
        let key = self.hint_key(font);
        // The cache is consulted by value: `Size` is `Copy` and the key is cheap to clone (its
        // font is a shared name), so no borrow of the cell outlives the comparison. The explicit
        // dereference clones the `Option`, not the `Ref` guard.
        let cached = (*self.hint_cache.borrow()).clone();
        if let Some((cached_key, size)) = cached {
            if cached_key == key {
                return size;
            }
        }
        let size = self.measure_implicit_size(font);
        *self.hint_cache.borrow_mut() = Some((key, size));
        size
    }

    /// The inputs the cached hint was derived from.
    pub(crate) fn hint_key(&self, font: &Font) -> ButtonHintKey {
        ButtonHintKey {
            text_revision: self.text_revision,
            font: font.clone(),
            icon: self.has_icon(),
            icon_size: self.icon_size,
            icon_position: self.icon_position,
            profile: active_profile(),
            shortcut: self.shortcut.clone(),
        }
    }

    /// Drops the cached intrinsic size.
    ///
    /// The key already distinguishes every input, so this is not needed for correctness; it makes
    /// the setters that own those inputs state their effect at the point they perform it, and it
    /// releases the cached `Font` reference when a control stops using its icon.
    fn invalidate_hint(&self) {
        let _ = self.hint_cache.replace(None);
        let _ = self.shortcut_hint_cache.replace(None);
    }

    /// Measures the intrinsic size from scratch; the slow path behind [`Button::implicit_size`].
    fn measure_implicit_size(&self, font: &Font) -> Size {
        // Measure with the effective font — the resolved theme/caller font, not the default — so
        // the intrinsic size matches the label the draw path paints with the same font
        // (D09-STYLE-02). A text scale or custom face now grows the hint with the ink.
        let line_height = estimate_line_height(font, 1.0);
        let label_width = estimate_text_width(&self.text, font, 1.0);
        // The icon is a content slot whichever side it is on. Leading and trailing slots take
        // horizontal room; top and bottom slots take a stacked row instead, so the label and the
        // icon never claim the same axis twice. Written as helpers rather than a `#[cfg]` pair of
        // expressions because the `image` feature is optional.
        let icon_extent = if self.has_icon() { self.icon_size } else { 0 };
        let (content_width, content_height) = match self.icon_position {
            IconPosition::Leading | IconPosition::Trailing => {
                (label_width + self.icon_advance(), line_height.max(self.icon_height()))
            }
            IconPosition::Top | IconPosition::Bottom => {
                (label_width.max(icon_extent), line_height + self.icon_height())
            }
        };
        // The profile, not the constant table: Material keeps its `64x40` floor and its 8 px
        // vertical padding, while macOS's `64x28` floor is the whole height and the remaining
        // air around the content is split evenly. Cupertino is the Material row. Compare the
        // profile's floor with the Material floor, not with the named padding: a tall custom
        // font must still grow a Material button instead of being squeezed into the 40 px
        // floor.
        let profile = profile_metrics(active_profile());
        let material_min = profile_metrics(ControlMetricsProfile::Material).button_min.height;
        let vertical_air = profile.button_min.height.saturating_sub(content_height) / 2;
        let padding_v = if profile.button_min.height < material_min {
            vertical_air
        } else {
            dimensions::BUTTON_PADDING_V
        };
        ControlMetrics::implicit_size(
            Size::new(content_width, content_height),
            EdgeOffsets::symmetric(padding_v, dimensions::BUTTON_PADDING_H),
            profile.button_min,
        )
    }

    /// How much horizontal room the icon and its gap take.
    ///
    /// Zero when there is no icon, when the `image` feature is off — in which case there is no
    /// icon field to read at all, so this is the honest answer rather than a special case — or
    /// when the icon is stacked above or below the label, where its room is vertical.
    fn icon_advance(&self) -> u32 {
        if self.has_icon()
            && matches!(self.icon_position, IconPosition::Leading | IconPosition::Trailing)
        {
            self.icon_size + dimensions::BUTTON_ICON_SPACING
        } else {
            0
        }
    }

    /// The height the icon contributes, or zero without one.
    ///
    /// A stacked icon contributes its own square plus the gap to the label, because the label
    /// follows it on the block axis. A leading or trailing icon only contributes the square,
    /// since it shares the label's line.
    fn icon_height(&self) -> u32 {
        if !self.has_icon() {
            return 0;
        }
        match self.icon_position {
            IconPosition::Top | IconPosition::Bottom => {
                self.icon_size + dimensions::BUTTON_ICON_SPACING
            }
            IconPosition::Leading | IconPosition::Trailing => self.icon_size,
        }
    }

    /// Whether an icon is present.
    ///
    /// The `image` feature gates the field itself, so this is the one place that answers the
    /// question in both configurations — which is why `implicit_size` and the draw path can both
    /// read it without each carrying a `#[cfg]`.
    fn has_icon(&self) -> bool {
        if self.icon_source.is_some() {
            return true;
        }
        #[cfg(feature = "image")]
        {
            self.icon.is_some() || {
                #[cfg(not(alloc_frugal))]
                {
                    self.shared_icon_pixels.is_some()
                }
                #[cfg(alloc_frugal)]
                {
                    false
                }
            }
        }
        #[cfg(not(feature = "image"))]
        {
            false
        }
    }

    /// The band the button's face actually occupies.
    ///
    /// `draw` clamps the painted height to `size_hint` and centres the result in the geometry,
    /// because a button's height is its own while its width is the layout's. The icon and label
    /// slots are derived from **that** band, not from the raw rectangle: deriving them from the
    /// full geometry put an `IconPosition::Top` icon at `y = 8` on a 200x120 button whose face
    /// painted at `y = 38..82`, so the icon floated above the pill it belongs to.
    pub(crate) fn painted_rect(&self) -> Rect {
        let rect = self.geometry();
        let painted_height = self.size_hint().height.min(rect.height).max(1);
        Rect::new(
            rect.x,
            rect.y + (rect.height.saturating_sub(painted_height) / 2) as i32,
            rect.width,
            painted_height,
        )
    }

    /// The box the icon occupies, when the button has one.
    ///
    /// The slot is the one [`Self::icon_position`] names. A leading icon is the historical
    /// picture: the icon square on the content box's first line, with the label following it by
    /// [`dimensions::BUTTON_ICON_SPACING`]. A trailing icon sits at the content box's far edge; a
    /// top or bottom icon is centred horizontally on its own row. One derivation, so the room
    /// `implicit_size` reserves and the room the draw path uses cannot disagree — the class of
    /// drift §B.9 names.
    ///
    /// For the two inline slots the vertical position is the **centred** one: an icon is not text
    /// and has no baseline, so it sits on the content box's middle line rather than on the text's.
    /// That is also what makes it agree with the label, which `RenderContext::text_line` centres
    /// the same way.
    pub fn icon_rect(&self) -> Option<Rect> {
        if !self.has_icon() {
            return None;
        }
        let content = ControlMetrics::content_box(
            self.painted_rect(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let size = self.icon_size.min(content.width).min(content.height);
        Some(match self.icon_position {
            IconPosition::Leading => ControlMetrics::center_in(
                Rect::new(content.x, content.y, size, content.height),
                Size::new(size, size),
            ),
            IconPosition::Trailing => ControlMetrics::center_in(
                Rect::new(
                    content.x + content.width as i32 - size as i32,
                    content.y,
                    size,
                    content.height,
                ),
                Size::new(size, size),
            ),
            IconPosition::Top => {
                let x = content.x + (content.width.saturating_sub(size) / 2) as i32;
                Rect::new(x, content.y, size, size)
            }
            IconPosition::Bottom => {
                let x = content.x + (content.width.saturating_sub(size) / 2) as i32;
                let y = content.y + content.height as i32 - size as i32;
                Rect::new(x, y, size, size)
            }
        })
    }

    /// The box the label occupies: the content box with the icon's own slot removed.
    ///
    /// A leading icon pushes the label's start right; a trailing icon shortens the label's end; a
    /// stacked icon removes a row from the top or the bottom. This is §B.9's "the padding is
    /// derived from the sibling's own size": neither slot overlaps the other.
    pub fn label_rect(&self) -> Rect {
        let content = ControlMetrics::content_box(
            self.painted_rect(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let advance = self.icon_advance().min(content.width);
        match self.icon_position {
            IconPosition::Leading => Rect::new(
                content.x + advance as i32,
                content.y,
                content.width - advance,
                content.height,
            ),
            IconPosition::Trailing => {
                Rect::new(content.x, content.y, content.width - advance, content.height)
            }
            IconPosition::Top => {
                // The icon box is clamped to the content box, so the label must use that same
                // clamped height; `icon_height()` measures the *declared* icon size and would
                // leave a gap in a button too short to fit it.
                let stacked = self
                    .icon_rect()
                    .map_or(0, |icon| icon.height + dimensions::BUTTON_ICON_SPACING)
                    .min(content.height);
                Rect::new(
                    content.x,
                    content.y + stacked as i32,
                    content.width,
                    content.height - stacked,
                )
            }
            IconPosition::Bottom => {
                let stacked = self
                    .icon_rect()
                    .map_or(0, |icon| icon.height + dimensions::BUTTON_ICON_SPACING)
                    .min(content.height);
                Rect::new(content.x, content.y, content.width, content.height - stacked)
            }
        }
    }

    /// The ink of the default-button emphasis ring.
    ///
    /// The theme's accent when one is active, because an emphasis has to read against a face the
    /// host may have styled arbitrarily; `bg`'s contrast colour is the fallback for a build with
    /// no palette, where no accent exists to read.
    pub(crate) fn emphasis_ring_color(&self, bg: Color) -> Color {
        crate::style::palette_shared()
            .as_ref()
            .map(|theme| theme.accent)
            .unwrap_or_else(|| bg.contrast_color())
    }

    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.base.is_pressed() {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }
    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.base.is_pressed()
    }
    /// Returns whether the pointer is currently over this button.
    ///
    /// Hover is recorded by [`BaseWidget`] from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    /// Forwarding to that single source keeps a button from holding a second,
    /// drifting opinion about hover; `ButtonState` still cannot carry it because
    /// hover and pressed are independent.
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }
    /// Sets the hovered flag and starts the interaction transition toward it.
    ///
    /// # Why this is public
    ///
    /// The `MouseEnter`/`MouseLeave` arms set the flag from real pointer events, but a host driving
    /// a control from its own input layer — a touch backend that has no hover concept, a test, or a
    /// designer previewing a state — needs a way to say "show me this control hovered". Without it
    /// the hovered appearance was reachable only by synthesising an event, which is a heavier and
    /// less honest way to state the same fact.
    ///
    /// Setting it also requests a redraw, because the flag is now something that changes what is
    /// painted over an animation rather than only at the next event.
    pub fn set_hovered(&mut self, hovered: bool) {
        // Delegate to the trait implementation, which writes the flag **and** re-resolves the
        // theme for `button:hover`; this control's implementation submits a narrow repaint.
        // This inherent method shadows the trait method, so an early return plus a plain field
        // write would make the key unreachable for this control. The fully qualified
        // `<dyn Widget>` names the implementation through the trait object, and its early
        // return is what keeps a repeated `MouseMove` cheap.
        <dyn Widget>::set_hovered(self, hovered);
    }
    /// Advances the interaction transition by `delta_ms` and reports whether another frame is needed.
    ///
    /// # The contract this implements
    ///
    /// The same one `FloatingLabel::tick` established: return `true` while there is still movement
    /// and `false` once the value has settled, so a host can stop scheduling frames for a control
    /// that is not moving. A `tick` that always returned `true` would keep the whole application
    /// repainting forever, which is why the boolean is part of the signature rather than something
    /// the caller infers.
    ///
    /// # Why the work is delegated to `PropertyDriver`
    ///
    /// The interpolation, the iteration counting and the engine's callback ownership are the
    /// same for every control that animates between two appearances. Keeping a private copy here
    /// meant the duration was read from `crate::theme` directly, which only exists in a build with
    /// a device profile — so this file failed to compile under `mini`/`embedded` until the read was
    /// routed through the `style` facade. `PropertyDriver` owns that read once.
    ///
    /// It also owns the **target**, so this method no longer writes a second field: the target is
    /// recomputed from the control's own state every tick (so a state change that arrived without a
    /// tick in between is picked up rather than missed) and handed to the driver, which keeps it.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // The target is recomputed every tick from the control's own state, so a state change
        // that arrived without a `tick` in between is picked up rather than missed.
        self.interaction_progress.set_target(self.interaction_target_progress());
        // Auto-repeat shares this control's existing clock: a held button reports one repeat per
        // elapsed interval, and no thread or platform timer is involved.
        let held = self.auto_repeat_held();
        let repeats = self.auto_repeat.advance(delta_ms, held);
        for _ in 0..repeats {
            self.emit_auto_repeat();
        }
        #[cfg_attr(not(full_widgets), allow(unused_mut))]
        let mut animating = self.interaction_progress.tick(delta_ms) || held;
        // A delayed popup is a held press, not an animation, but it still owes the bus a frame:
        // without that the timer would only fire when some other control happened to tick.
        #[cfg(full_widgets)]
        if let Some(elapsed) = self.press_elapsed.as_mut() {
            *elapsed = elapsed.saturating_add(delta_ms);
            if *elapsed >= self.popup_delay_ms {
                self.press_elapsed = None;
                self.open_menu();
            }
            animating = true;
        }
        animating
    }

    /// Whether this button is held and configured to repeat.
    ///
    /// Asks the *configuration* rather than `AutoRepeat::is_active`, because the timer has not
    /// started before the first tick: an `is_animating` that answered `false` here would keep the
    /// animation bus from ever starting the ticks the timer needs.
    fn auto_repeat_held(&self) -> bool {
        self.auto_repeat.is_enabled() && self.base.is_enabled() && self.base.is_pressed()
    }

    /// Reports one auto-repeat cycle.
    ///
    /// A repeat is a `released`/`pressed` pair on this button's own signals, which is the cycle a
    /// held button emits in the reference toolkit: the latch never moves, so the button stays
    /// painted pressed while its listeners see each repeat. The pair is also what the group
    /// reports, so an observer of either level sees the same sequence.
    fn emit_auto_repeat(&mut self) {
        self.released_signal.emit();
        self.pressed_signal.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
    }

    /// The progress the current interaction state calls for.
    ///
    /// A press is the furthest point of the transition, a hover a partial step toward it, and a
    /// disabled control always rest. Disabled wins over pressed because a control that became
    /// disabled mid-press is inert, which is the same precedence `Button::state` uses.
    pub(crate) fn interaction_target_progress(&self) -> f32 {
        if !self.base.is_enabled() {
            return 0.0;
        }
        if self.base.is_pressed() {
            1.0
        } else if self.base.is_hovered() {
            0.5
        } else {
            0.0
        }
    }

    /// Sets pressed state and emits transition signals when changed.
    ///
    /// Ignored entirely (no state change, no signals) while the button is
    /// disabled. On a real change it emits `pressed_signal` or
    /// `released_signal` followed by `state_changed`, and requests a redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        // The trait implementation carries both the field write and the theme re-resolution for
        // `button:pressed`; see `Button::set_hovered` for why an inherent method must delegate
        // through the trait object rather than writing the field itself.
        <dyn Widget>::set_pressed(self, pressed);
        if self.signaled_pressed == pressed {
            return;
        }
        self.signaled_pressed = pressed;
        // A new gesture starts its repeat delay from zero, even when the press arrived through the
        // property layer rather than through `press()`.
        self.auto_repeat.reset();
        if pressed {
            self.pressed_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
        } else {
            self.released_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        }
        self.state_changed.emit(self.state());
        self.request_frame_repaint(None);
    }
    /// Begins a gesture: takes the grab and marks the control pressed.
    ///
    /// The grab is taken **before** the pressed flag so that `set_pressed(false)`
    /// during an ungrab cannot re-enter and drop a grab that was never taken.
    pub fn press(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        // A new gesture starts its auto-repeat delay from zero, even if the previous gesture's
        // clock never got a tick that observed it as released.
        self.auto_repeat.reset();
        // A press on a menu button also starts the delayed-popup timer. A button with no menu
        // keeps the historical path exactly: the timer is never armed.
        #[cfg(full_widgets)]
        if !self.menu_items.is_empty() {
            self.press_elapsed = Some(0);
        }
        self.base.set_grabbed(true);
        self.set_pressed(true);
    }
    /// Ends a gesture: drops the grab and clears the pressed flag.
    pub fn release(&mut self) {
        self.auto_repeat.reset();
        #[cfg(full_widgets)]
        {
            self.press_elapsed = None;
        }
        self.base.set_grabbed(false);
        self.set_pressed(false);
    }

    /// Abandons a gesture without activating: ends it and emits `canceled` if it was live.
    ///
    /// This is the standard ungrab handling: it is the single place that decides what
    /// "the interaction is off" means. Called from a focus change and from disabling the
    /// control — so the paths cannot drift into slightly different notions of cancellation.
    pub fn cancel_gesture(&mut self) {
        if !self.base.is_grabbed() && !self.base.is_pressed() {
            return;
        }
        self.auto_repeat.reset();
        #[cfg(full_widgets)]
        {
            self.press_elapsed = None;
        }
        self.base.set_grabbed(false);
        if self.base.is_enabled() {
            self.set_pressed(false);
        } else if self.base.is_pressed() || self.signaled_pressed {
            // `set_pressed` is inert while disabled, and `set_enabled_state(false)` disables
            // **before** it cancels. Routing cancellation through it there left the painted
            // latch and the `signaled_pressed` edge set, so re-enabling the button reported
            // `Pressed` with no pointer down and the next press emitted no `pressed`. The
            // gesture is over either way, so clear both directly and report the release the
            // disabled path could not emit.
            self.base.set_pressed(false);
            if self.signaled_pressed {
                self.signaled_pressed = false;
                self.released_signal.emit();
                button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
            }
        }
        self.canceled.emit();
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19).
    ///
    /// The emitted field is the US spelling; this accessor returns the **same**
    /// signal, so a subscriber that used the double-l name hears the emitted event.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// Whether this control currently owns the pointer gesture.
    pub fn is_grabbed(&self) -> bool {
        self.base.is_grabbed()
    }
    /// Enables/disables button while preserving deterministic state transitions.
    ///
    /// Disabling mid-gesture abandons it, emitting `canceled`: a control that became
    /// inert while held will never receive the release that would have ended the
    /// gesture normally, so leaving the grab set would keep a stale `pressed` paint and
    /// an arm that fires on an unrelated later release. The transient
    /// state is cleared at the same moment (`button_style_button.dart:359-362`).
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        let was_enabled = self.base.is_enabled();
        self.base.set_enabled(enabled);
        if !enabled {
            self.cancel_gesture();
        }
        // A disabled button resolves `button:disabled`, which the theme layer can only do
        // through the control: re-resolve here rather than leaving the fill of the previous
        // state on screen. Guarded on an actual change, so a repeated call is free.
        if was_enabled != enabled {
            self.set_state_theme_hook();
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
            self.base.request_redraw();
            // A state change is also a change a screen reader should hear about (ENG-A4).
            #[cfg(not(alloc_frugal))]
            if let Some(id) = crate::widget::runtime::registry_id_of(self.base.id()) {
                crate::widget::a11y_submit::submit_state_changed(id);
            }
        }
    }
    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            // The caption is an input to the intrinsic size; bumping the revision is what makes
            // the cached answer invalid without an explicit `invalidate` call here.
            self.text_revision = self.text_revision.wrapping_add(1);
            self.invalidate_hint();
            self.base.request_redraw();
        }
    }
    /// Sets the icon displayed on the button.
    #[cfg(feature = "image")]
    pub fn set_icon(&mut self, icon: Image) {
        self.icon_source = None;
        #[cfg(not(alloc_frugal))]
        {
            self.shared_icon_pixels = None;
        }
        self.icon = Some(icon);
        self.icon_tint.clear();
        self.invalidate_hint();
        self.base.request_redraw();
    }
    /// Returns a reference to the button icon, if set.
    #[cfg(feature = "image")]
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }

    /// Sets immutable RGBA8 icon pixels shared with other controls.
    ///
    /// The handle clone is constant-time; use
    /// [`crate::widget::icon_source::SharedIconPixels::from_image`] to move a decoded RGBA8 image
    /// into shared storage without copying its pixel vector.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn set_shared_icon_pixels(
        &mut self,
        pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    ) {
        self.shared_icon_pixels = pixels;
        self.icon = None;
        self.icon_source = None;
        self.icon_tint.clear();
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// Returns the shared pixel handle, if one is active.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn shared_icon_pixels(&self) -> Option<&crate::widget::icon_source::SharedIconPixels> {
        self.shared_icon_pixels.as_ref()
    }

    /// Sets a built-in or static-geometry icon, replacing any legacy image.
    ///
    /// The legacy [`Self::icon`] getter returns `None` while a typed source is active; use
    /// [`Self::icon_source`] to inspect the current source.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
        #[cfg(feature = "image")]
        {
            self.icon = None;
        }
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.shared_icon_pixels = None;
        }
        self.icon_tint.clear();
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// Returns the typed icon source, if one was most recently assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Returns the colour the icon is tinted with, if any.
    pub fn icon_tint(&self) -> Option<crate::core::Color> {
        self.icon_tint.tint()
    }

    /// Sets or clears the icon's tint colour.
    ///
    /// # What is tinted
    ///
    /// * A vector icon ([`IconSource`]) is painted in this colour instead of the label's ink.
    /// * An image — [`Self::set_shared_icon_pixels`] or the legacy [`Self::set_icon`] — is treated
    ///   as a **monochrome mask**: its alpha is kept and its RGB is replaced by the tint. That is
    ///   the reading [`IconColorMode::Monochrome`](crate::widget::icon_source::IconColorMode)
    ///   already declares for a tintable resource; a photograph should keep `None` rather than lose
    ///   its colours.
    ///
    /// `None` restores the pre-existing behaviour exactly: a vector icon follows the label ink, an
    /// image is drawn with its own colours, and no copy is composed.
    pub fn set_icon_tint(&mut self, tint: Option<crate::core::Color>) {
        if self.icon_tint.set(tint) {
            self.base.request_redraw();
        }
    }
    /// Sets whether this button is the default button.
    pub fn set_default(&mut self, default: bool) {
        let already_default = self.default_button == default;
        self.default_button = default;
        if self.auto_exclusive {
            if default {
                // A repeat "default" on an auto-exclusive button may still need to claim the group
                // selection once, which is the same rule the radio family follows.
                self.claim_group_selection();
            } else if let Some(key) = self.exclusive_key() {
                // Clearing the flag on the emphasised member clears the group's selection, so
                // `is_default` does not keep answering `true` from the stale selection.
                if button_group::is_selected(&key, self.id()) {
                    button_group::clear_selection(&key);
                }
            }
        }
        if !already_default {
            self.base.request_redraw();
        }
    }

    /// Returns whether this button is the default button.
    ///
    /// An auto-exclusive button's answer comes from its group: the member last made default (or
    /// clicked) is the one the emphasis ring marks, and its siblings are not.
    pub fn is_default(&self) -> bool {
        if self.auto_exclusive {
            if let Some(key) = self.exclusive_key() {
                if let Some(selected) = button_group::selected(&key) {
                    return selected == self.id();
                }
            }
        }
        self.default_button
    }

    /// The icon slot's size in logical pixels.
    pub fn icon_size(&self) -> u32 {
        self.icon_size
    }

    /// Where the icon sits relative to the label.
    pub fn icon_position(&self) -> IconPosition {
        self.icon_position
    }

    /// Sets where the icon sits relative to the label.
    ///
    /// The room the intrinsic size reserves and the boxes the draw path paints both read this
    /// one value, so a changed position cannot make the two disagree. The default is
    /// [`IconPosition::Leading`], which keeps the historical picture.
    pub fn set_icon_position(&mut self, position: IconPosition) {
        if self.icon_position == position {
            return;
        }
        self.icon_position = position;
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// Sets the icon slot's size.
    ///
    /// The room the intrinsic size reserves and the square the draw path paints both read this
    /// one value, so a custom icon size cannot make the two disagree.
    pub fn set_icon_size(&mut self, size: u32) {
        if self.icon_size == size {
            return;
        }
        self.icon_size = size;
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// The keyboard chord that activates this button, if one was declared.
    pub fn shortcut(&self) -> Option<crate::shortcut::Shortcut> {
        self.shortcut.clone()
    }

    /// Declares the keyboard chord that activates this button.
    ///
    /// The button checks it in its own `KeyPress` arm, so a host that delivers key events to its
    /// focused control gets shortcut activation without a separate application-level table. A
    /// process-wide shortcut map is still the right place for accelerators that must fire while
    /// another control has focus.
    pub fn set_shortcut(&mut self, shortcut: Option<crate::shortcut::Shortcut>) {
        if self.shortcut == shortcut {
            return;
        }
        self.shortcut = shortcut;
        self.base.request_redraw();
    }

    /// Whether focus makes this button the default button.
    pub fn is_auto_default(&self) -> bool {
        self.auto_default
    }

    /// Makes gaining focus set the default flag, and losing it clear the flag.
    ///
    /// This is the reference toolkit's `autoDefault`: in a dialog, the button the keyboard is on
    /// is the one Enter presses. An explicit [`Button::set_default`] remains available and writes
    /// the same flag, so the two cannot disagree about what "default" means.
    pub fn set_auto_default(&mut self, auto_default: bool) {
        if self.auto_default == auto_default {
            return;
        }
        self.auto_default = auto_default;
        if !auto_default && self.is_default() {
            self.set_default(false);
        }
        self.base.request_redraw();
    }

    /// Whether this button's default emphasis is exclusive with its siblings.
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }

    /// Groups this button with the other auto-exclusive buttons under the same parent.
    ///
    /// A push button has no latch for `autoExclusive` to guard, so the exclusive fact here is the
    /// **default emphasis**: clicking one member of the group moves the emphasis to it, which is
    /// the visible "this is the button Enter presses" state. Off by default; a lone button with no
    /// parent has no peers and is therefore unaffected.
    pub fn set_auto_exclusive(&mut self, exclusive: bool) {
        if self.auto_exclusive == exclusive {
            return;
        }
        let previous_key = self.exclusive_key();
        self.auto_exclusive = exclusive;
        if let Some(key) = previous_key {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
        self.base.request_redraw();
    }

    /// Whether the button paints without a face or frame.
    pub fn is_flat(&self) -> bool {
        self.flat
    }

    /// Draws the button as a flat, chrome-less control.
    ///
    /// A flat button paints only its content until the pointer interacts with it, which is the
    /// reference toolkit's `flat` property. Off by default so the existing appearance is
    /// untouched.
    pub fn set_flat(&mut self, flat: bool) {
        if self.flat == flat {
            return;
        }
        self.flat = flat;
        self.base.request_redraw();
    }

    /// How a label that does not fit the button is shortened.
    pub fn elide_mode(&self) -> TextElideMode {
        self.elide_mode
    }

    /// Sets the label's elide mode.
    pub fn set_elide_mode(&mut self, mode: TextElideMode) {
        if self.elide_mode == mode {
            return;
        }
        self.elide_mode = mode;
        self.base.request_redraw();
    }

    /// Whether holding this button repeats its press/release cycle.
    pub fn is_auto_repeat(&self) -> bool {
        self.auto_repeat.is_enabled()
    }

    /// Enables automatic repetition while the button is held.
    ///
    /// Off by default, so an existing button still activates once per press. When on, the button's
    /// own `pressed`/`released` signals are re-emitted at [`Self::auto_repeat_interval`] once the
    /// press has been held for [`Self::auto_repeat_delay`] — the reference toolkit's `autoRepeat`.
    pub fn set_auto_repeat(&mut self, enabled: bool) {
        if self.auto_repeat.set_enabled(enabled) {
            self.base.request_redraw();
        }
    }

    /// How long the button must be held before the first repeat, in milliseconds.
    pub fn auto_repeat_delay(&self) -> u32 {
        self.auto_repeat.delay_ms()
    }

    /// Sets the delay before the first repeat.
    pub fn set_auto_repeat_delay(&mut self, delay_ms: u32) {
        self.auto_repeat.set_delay_ms(delay_ms);
    }

    /// The gap between repeats while the button is held, in milliseconds.
    pub fn auto_repeat_interval(&self) -> u32 {
        self.auto_repeat.interval_ms()
    }

    /// Sets the gap between repeats.
    pub fn set_auto_repeat_interval(&mut self, interval_ms: u32) {
        self.auto_repeat.set_interval_ms(interval_ms);
    }

    /// Whether a press on this button can start a cross-control drag.
    pub fn is_drag_enabled(&self) -> bool {
        self.drag_enabled
    }

    /// Enables the button as a drag source.
    ///
    /// The payload is the button's own caption as `text/plain`, so it can land in any text
    /// control. Off by default: a push button that suddenly became draggable when the runtime
    /// grew a drag coordinator would change the meaning of every existing press.
    pub fn set_drag_enabled(&mut self, enabled: bool) {
        if self.drag_enabled == enabled {
            return;
        }
        self.drag_enabled = enabled;
        self.base.request_redraw();
    }

    /// Whether this button accepts dropped payloads.
    pub fn accepts_drops(&self) -> bool {
        self.accepts_drops
    }

    /// Enables the button as a drop target for `text/plain` payloads.
    ///
    /// A drop replaces the button's caption. Off by default, matching the reference toolkit's
    /// `setAcceptDrops` opt-in.
    pub fn set_accepts_drops(&mut self, accepts: bool) {
        if self.accepts_drops == accepts {
            return;
        }
        self.accepts_drops = accepts;
        if !accepts {
            self.drop_hovered = false;
        }
        self.base.request_redraw();
    }

    /// The group this button belongs to, when auto-exclusive.
    ///
    /// A group a [`crate::widget::base_widgets::ButtonGroup`] handle adopted this button into
    /// always wins: it is a direct instruction from the caller, while `auto_exclusive` is a
    /// property that can only describe a same-parent group.
    fn exclusive_key(&self) -> Option<GroupKey> {
        if let Some(key) = button_group::membership(self.id()) {
            return Some(key);
        }
        if !self.auto_exclusive {
            return None;
        }
        self.parent().map(GroupKey::Parent)
    }

    /// Records this button's current facts with its group.
    fn sync_group_registration(&self) {
        if let Some(key) = self.exclusive_key() {
            button_group::register(key, self.id(), self.geometry(), self.base.is_enabled());
        }
    }

    /// Claims the group's emphasis for a pointer or touch activation.
    ///
    /// A menu button's activation opens its menu rather than selecting the button, which is the
    /// same exception the keyboard arm makes, so only a plain button claims here.
    fn claim_pointer_activation(&mut self) {
        if !self.auto_exclusive {
            return;
        }
        #[cfg(full_widgets)]
        if !self.menu_items.is_empty() {
            return;
        }
        self.claim_group_selection();
    }

    /// Makes this button the group's emphasised member.
    fn claim_group_selection(&mut self) {
        let Some(key) = self.exclusive_key() else {
            return;
        };
        button_group::register(key.clone(), self.id(), self.geometry(), self.base.is_enabled());
        button_group::select(&key, self.id());
        self.base.request_redraw();
    }

    /// The attached menu's rows.
    #[cfg(full_widgets)]
    pub fn menu_items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Attaches (or replaces) this button's drop-down menu.
    ///
    /// An empty list removes the menu. The button then behaves exactly as it did before: a click
    /// emits `clicked` and nothing is painted below it.
    #[cfg(full_widgets)]
    pub fn set_menu(&mut self, items: Vec<MenuItem>) {
        self.menu_items = items;
        if self.menu_items.is_empty() {
            self.close_menu();
        }
        self.base.request_redraw();
    }

    /// Adds one row to the attached menu.
    #[cfg(full_widgets)]
    pub fn add_menu_item(&mut self, item: MenuItem) {
        self.menu_items.push(item);
        self.base.request_redraw();
    }

    /// Removes every row from the attached menu and closes it.
    #[cfg(full_widgets)]
    pub fn clear_menu(&mut self) {
        if self.menu_items.is_empty() {
            return;
        }
        self.menu_items.clear();
        self.close_menu();
        self.base.request_redraw();
    }

    /// Whether the attached menu is open.
    #[cfg(full_widgets)]
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// The rectangle the open menu occupies.
    #[cfg(full_widgets)]
    pub fn menu_rect(&self) -> Rect {
        let anchor = match self.menu_anchor {
            Some(point) => Rect::new(point.x, point.y, 0, 0),
            None => self.geometry(),
        };
        menu_panel_rect(anchor, self.menu_items.len())
    }

    /// Opens the attached menu below the button.
    #[cfg(full_widgets)]
    pub fn open_menu(&mut self) {
        if self.menu_items.is_empty() || self.menu_open {
            return;
        }
        self.menu_open = true;
        self.menu_anchor = None;
        self.active_menu_index = self.menu_items.iter().position(|item| item.enabled);
        // The open menu owns the press that dismisses it, as every other menu owner does
        // (`MenuButton`, `ToolButton`, `DropdownMenu`). Without the grab an outside press went
        // to the control behind, which both fired it and left this menu open.
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Opens the attached menu at `pos`, which is how a context-menu request anchors it.
    #[cfg(full_widgets)]
    pub fn open_menu_at(&mut self, pos: Point) {
        if self.menu_items.is_empty() {
            return;
        }
        self.menu_open = true;
        self.menu_anchor = Some(pos);
        self.active_menu_index = self.menu_items.iter().position(|item| item.enabled);
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Closes the attached menu.
    #[cfg(full_widgets)]
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.menu_anchor = None;
        self.active_menu_index = None;
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
    }

    /// Toggles the attached menu.
    #[cfg(full_widgets)]
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// How long a press must be held before the attached menu opens.
    #[cfg(full_widgets)]
    pub fn popup_delay_ms(&self) -> u32 {
        self.popup_delay_ms
    }

    /// Sets the delayed-popup hold time.
    ///
    /// Only a button with an attached menu arms the timer, so this has no observable effect
    /// until [`Self::set_menu`] has been given rows.
    #[cfg(full_widgets)]
    pub fn set_popup_delay_ms(&mut self, delay_ms: u32) {
        self.popup_delay_ms = delay_ms;
    }

    /// The enabled menu row at `pos`, if any.
    #[cfg(full_widgets)]
    fn hit_test_menu_item(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let panel = self.menu_rect();
        if !panel.contains_point(pos) {
            return None;
        }
        self.menu_items.iter().enumerate().find_map(|(index, item)| {
            let row = menu_item_rect(panel, index);
            (item.enabled && row.contains_point(pos)).then_some(index)
        })
    }

    /// Activates the menu row at `index`: emits `item_triggered` and closes the menu.
    #[cfg(full_widgets)]
    fn activate_menu_row(&mut self, index: usize) {
        let Some(item) = self.menu_items.get(index) else {
            return;
        };
        if !item.enabled {
            return;
        }
        let id = item.id;
        self.item_triggered.emit(id);
        self.close_menu();
    }

    /// Moves the keyboard highlight through the menu's enabled rows.
    ///
    /// Index-based rather than collecting the enabled rows into a fresh `Vec` on every key:
    /// the menu's own vector is the entry list, so a held arrow key costs one walk and no
    /// allocation. The walk wraps and skips disabled rows, which is the behaviour the
    /// collected version had.
    #[cfg(full_widgets)]
    pub(crate) fn move_menu_highlight(&mut self, forward: bool) {
        let next = match self.active_menu_index {
            Some(active) if self.menu_items.get(active).is_some_and(|item| item.enabled) => {
                self.step_enabled_index(active, forward)
            }
            _ => self.first_enabled_index(),
        };
        if let Some(index) = next {
            self.active_menu_index = Some(index);
            self.base.request_redraw();
        }
    }

    /// The first enabled row, or `None` when every row is disabled.
    #[cfg(full_widgets)]
    fn first_enabled_index(&self) -> Option<usize> {
        self.menu_items.iter().position(|item| item.enabled)
    }

    /// The next enabled row from `from`, wrapping and skipping disabled rows.
    #[cfg(full_widgets)]
    fn step_enabled_index(&self, from: usize, forward: bool) -> Option<usize> {
        let len = self.menu_items.len();
        if len == 0 {
            return None;
        }
        let mut index = from;
        for _ in 0..len {
            index = if forward { (index + 1) % len } else { (index + len - 1) % len };
            if self.menu_items[index].enabled {
                return Some(index);
            }
        }
        None
    }

    /// Programmatically clicks the button (press, release, emit clicked signal).
    pub fn click(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        self.press();
        self.release();
        if self.auto_exclusive {
            self.claim_group_selection();
        }
        self.emit_clicked();
    }

    /// Emits `clicked` and reports the click to this button's group, if it has one.
    ///
    /// Every activation path goes through here rather than emitting the base signal directly, so
    /// a pointer release, a key activation, a programmatic `click`, and a touch tap cannot
    /// disagree about whether the group was told.
    fn emit_clicked(&mut self) {
        self.base.clicked.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Clicked);
    }

    /// Whether `key`/`modifiers` is this button's declared shortcut.
    ///
    /// The modifier set must match exactly: a `Ctrl+S` shortcut does not fire on `Ctrl+Shift+S`,
    /// which is a different binding that the caller did not declare. Both halves go through the
    /// crate's existing key vocabulary rather than raw numbers, so a platform mapping change
    /// cannot make a declared shortcut unreachable.
    fn shortcut_matches(&self, key: u32, modifiers: u32) -> bool {
        let Some(shortcut) = self.shortcut.as_ref() else {
            return false;
        };
        let Some(pressed) = crate::shortcut::Key::from_key_code(key) else {
            return false;
        };
        if shortcut.key != pressed {
            return false;
        }
        // The event bitmask is the framework's own convention, so the comparison works on those
        // bits rather than on the `Modifiers` bits: `Modifiers::PRIMARY` has no event bit of its
        // own (it is the *meaning* of the Command/Meta bit), which is why a naive equality between
        // the two representations would never match a `Shortcut::primary` binding.
        use crate::shortcut::Modifiers;
        let shift = modifiers & 0b0001 != 0;
        let ctrl = modifiers & 0b0010 != 0;
        let alt = modifiers & 0b0100 != 0;
        let meta = modifiers & 0b1000 != 0;
        if shortcut.modifiers.contains(Modifiers::SHIFT) != shift {
            return false;
        }
        if shortcut.modifiers.contains(Modifiers::ALT) != alt {
            return false;
        }
        // A "primary" declaration accepts the host's accelerator bit: Command on macOS, Control
        // on the other platforms. A physical-Control declaration accepts only the Control bit.
        let wants_ctrl_like = shortcut.modifiers.contains(Modifiers::PRIMARY)
            || shortcut.modifiers.contains(Modifiers::CTRL);
        if wants_ctrl_like != (ctrl || meta) {
            return false;
        }
        // The Meta event bit doubles as the primary accelerator, so a declared `META` cannot be
        // told apart from a declared `PRIMARY`; both accept it rather than one silently failing.
        true
    }

    /// Submits one animated or interactive frame as damage bounded by the painted box
    /// plus the focus-ring and halo margin.
    ///
    /// `bounds` is `Some` only when the caller already has the rectangle; interaction
    /// paths pass `None` and let the runtime derive the box.
    ///
    /// A control that is unmounted falls back to [`BaseWidget::request_redraw`], which
    /// keeps the redraw signal contract for host-owned controls. An interaction frame in
    /// [`crate::widget::runtime::RepaintMode::Full`] still routes through
    /// [`crate::widget::runtime::request_repaint_rect`], whose documented fallback repaints
    /// the whole control.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                // Interaction paths may take the whole-control fallback in `Full`;
                // `bounds` lets a caller that already has the rectangle skip the lookup.
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    // The replaced `request_redraw` emitted this signal; keep it.
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This control's own rectangle grown by
    /// [`crate::widget::metrics::DAMAGE_MARGIN`].
    ///
    /// Used when the runtime cannot read the mounted geometry back, which is the case
    /// while an interaction event holds the control out of the registry. Deriving the
    /// rectangle from `Widget::geometry` yields the same answer the runtime returns for
    /// the mounted control.
    #[cfg(not(alloc_frugal))]
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }
}
impl Widget for Button {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// # The mapping
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `clicked` | `base.clicked` | none |
    /// | `pressed` | `pressed_signal` | none |
    /// | `released` | `released_signal` | none |
    /// | `state_changed` | `state_changed` | `ButtonState` as a token string |
    ///
    /// The names are the ones `button_capability` publishes, so a name `connect_event` accepts is a
    /// name that resolves here — that correspondence is what `tools/check_event_signal_dyn.sh`
    /// verifies for every converted control.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            // `clicked` lives on the base, so every control has it; the `Widget` trait exposes it
            // whether or not the concrete type does anything with it.
            "clicked" => Some(EventSignalRef::unit("clicked", self.clicked_signal())),
            "pressed" => Some(EventSignalRef::unit("pressed", &self.pressed_signal)),
            "released" => Some(EventSignalRef::unit("released", &self.released_signal)),
            // A gesture abandoned by a release outside the button, or by an ungrab. It is a
            // distinct event from `released`: the button did not activate, so a host that
            // wants to run 「the press was abandoned」 logic needs its own subscription.
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            // W-19 compatibility spelling. It declares its own name so a subscriber that used the
            // old spelling is attached to a reference that answers for exactly that spelling.
            // An enum payload travels as its token spelling, which is the same representation the
            // property side uses for `PropertyValueKind::Enum` — one spelling, not two.
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |state| {
                    crate::widget::capability::CapabilityValue::String(format!("{state:?}"))
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |pos| {
                    crate::widget::capability::CapabilityValue::String(format!(
                        "{},{}",
                        pos.x, pos.y
                    ))
                },
            )),
            #[cfg(full_widgets)]
            "item_triggered" => {
                Some(EventSignalRef::mapped("item_triggered", &self.item_triggered, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }

    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Records whether the incoming style was authored by the caller or by the theme.
    ///
    /// `theme_derived` is the style record's own statement of "the theme wrote this", so it is
    /// the explicit authorship flag the corner-radius decision needs. Reading it here, at the
    /// one place a whole style is installed, keeps the rule out of `draw`.
    fn set_style(&mut self, style: crate::style::WidgetStyle) {
        self.border_radius_explicit = style.border_radius.is_some() && !style.theme_derived;
        self.base.set_style(style);
    }

    /// Sets the corner radius and marks it caller-authored.
    ///
    /// The default trait method writes only the field, which would leave the button unable to
    /// tell a caller's explicit value from the theme's baseline on a later redraw.
    fn set_border_radius(&mut self, radius: u32) {
        self.border_radius_explicit = true;
        let style = self.base.style_mut();
        style.border_radius = Some(radius);
        // The caller is now a co-author, so a later theme application may only fill fields
        // this style left unset rather than replacing the radius it just chose.
        style.theme_derived = false;
    }

    /// Sets a whole border and marks the radius as caller-authored.
    ///
    /// The default implementation clones the current style, which on a themed button carries
    /// `theme_derived = true`; keeping that flag would let the next theme application replace
    /// the radius the caller just wrote.
    fn set_border(&mut self, color: Option<Color>, width: u32, radius: u32) {
        let mut style = self.style().clone();
        style.border_color = color;
        style.border_width = Some(width);
        style.border_radius = Some(radius);
        style.theme_derived = false;
        self.set_style(style);
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.set_enabled_state(enabled);
        self.sync_group_registration();
    }

    /// Keeps the exclusive group's geometry snapshot current.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.sync_group_registration();
    }

    /// Keeps the exclusive group in step with the parent it is keyed by.
    fn set_parent(&mut self, parent: Option<crate::core::ObjectId>) {
        let previous = self.exclusive_key();
        self.base.set_parent(parent);
        if let Some(key) = previous {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
    }

    /// The button, and its open menu, accept the pointer.
    ///
    /// The menu is painted outside the button's own rectangle, so without this override the
    /// runtime would hit-test a click on a menu row to whatever is behind the button.
    fn contains_point(&self, point: Point) -> bool {
        #[cfg(full_widgets)]
        if self.menu_open && self.menu_rect().contains_point(point) {
            return true;
        }
        self.base.contains_point_with_touch_expansion(point)
    }

    #[cfg(full_widgets)]
    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    /// Turns a context-menu request into a menu, either its own or a request to the host.
    ///
    /// A button with an attached menu opens it at the pointer. A button without one emits
    /// [`Self::context_menu_requested`] and answers `true`: the request is about this button, so a
    /// host that subscribed acts on it, and one that did not asked for no menu. This is the same
    /// division the list and table controls use.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        #[cfg(full_widgets)]
        if !self.menu_items.is_empty() {
            self.open_menu_at(pos);
            return true;
        }
        self.context_menu_requested.emit(pos);
        true
    }

    /// A press on a draggable button carries its caption.
    fn drag_source_mut(&mut self) -> Option<&mut dyn crate::event::dnd::DragSource> {
        (self.drag_enabled && self.base.is_enabled() && !self.text.is_empty()).then_some(self)
    }

    /// A button that accepts drops receives `text/plain` payloads.
    fn drop_target_mut(&mut self) -> Option<&mut dyn crate::event::dnd::DropTarget> {
        (self.accepts_drops && self.base.is_enabled()).then_some(self)
    }

    /// Sets hover and repaints only the area this control can change.
    ///
    /// The flag write and theme re-resolution match the trait default; the repaint is
    /// the narrow one, so a hover does not invalidate the whole control.
    fn set_hovered(&mut self, hovered: bool) {
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
        self.set_state_theme_hook();
        self.request_frame_repaint(None);
    }

    /// Sets pressed and repaints only the area this control can change.
    ///
    /// The flag write and theme re-resolution match the trait default; the repaint is
    /// the narrow one, so a press does not invalidate the whole control.
    fn set_pressed(&mut self, pressed: bool) {
        if self.base.is_pressed() == pressed {
            return;
        }
        self.base.set_pressed(pressed);
        self.set_state_theme_hook();
        self.request_frame_repaint(None);
    }

    fn size_hint(&self) -> Size {
        // Delegates to the metric-driven derivation; the `debug_assert!` names the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for at a one-line delegation site.
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    // The interaction transition is what makes a hover *fade* rather than snap, so this
    // control owes frames while it is moving. `Button::tick` is the inherent method that
    // owns the interpolation; this is the trait spelling a host holding `&mut dyn Widget`
    // can reach, which is what the animation bus drives.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Button::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // Derived from the control's *state*, not from the tick-time target field: a state
        // change (a hover arriving) makes the control animating at once, before any `tick`
        // has run to recompute the target. Reading the stale field made `is_animating`
        // answer `false` for a button that had just been hovered, so the bus would never
        // start its frames. A held auto-repeating button owes frames for the same reason, and
        // for the same reason it is asked of the configuration rather than of the timer.
        #[cfg(full_widgets)]
        if self.press_elapsed.is_some() {
            return true;
        }
        self.interaction_progress.value() != self.interaction_target_progress()
            || self.auto_repeat_held()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Button`'s property contract.
///
/// This is the reference implementation for the property layer: a control names
/// its own properties here, reads and writes them against its own fields, and
/// forwards every name it does not recognise to the shared base helpers. Adding a
/// property to a control means editing this block and nothing else — the read
/// path, write path and published names all come from it.
impl WidgetProperties for Button {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "pressed" => Ok(CapabilityValue::Bool(self.is_pressed())),
            "default" => Ok(CapabilityValue::Bool(self.is_default())),
            "icon_size" => Ok(CapabilityValue::UInt(self.icon_size() as u64)),
            "icon_position" => {
                Ok(CapabilityValue::String(self.icon_position().as_str().to_string()))
            }
            "shortcut" => match self.shortcut() {
                Some(shortcut) => Ok(CapabilityValue::String(shortcut.format_shortcut())),
                None => Ok(CapabilityValue::Null),
            },
            "auto_default" => Ok(CapabilityValue::Bool(self.is_auto_default())),
            "auto_exclusive" => Ok(CapabilityValue::Bool(self.is_auto_exclusive())),
            "flat" => Ok(CapabilityValue::Bool(self.is_flat())),
            "elide" => Ok(CapabilityValue::String(self.elide_mode().as_str().to_string())),
            // Auto-repeat is a behaviour of the press gesture, so the three knobs travel through
            // the same property route as the gesture's other settings.
            "auto_repeat" => Ok(CapabilityValue::Bool(self.is_auto_repeat())),
            "auto_repeat_delay" => Ok(CapabilityValue::UInt(self.auto_repeat_delay() as u64)),
            "auto_repeat_interval" => Ok(CapabilityValue::UInt(self.auto_repeat_interval() as u64)),
            "drag_enabled" => Ok(CapabilityValue::Bool(self.is_drag_enabled())),
            "accepts_drops" => Ok(CapabilityValue::Bool(self.accepts_drops())),
            #[cfg(full_widgets)]
            "menu_open" => Ok(CapabilityValue::Bool(self.is_menu_open())),
            #[cfg(full_widgets)]
            "menu_item_count" => Ok(CapabilityValue::UInt(self.menu_items.len() as u64)),
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            "icon_tint" => Ok(match self.icon_tint() {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "pressed" => {
                self.set_pressed(expect_bool(value)?);
                Ok(())
            }
            "default" => {
                self.set_default(expect_bool(value)?);
                Ok(())
            }
            "icon_size" => {
                self.set_icon_size(expect_u32(value)?);
                Ok(())
            }
            "icon_position" => {
                let token = expect_string(value)?;
                let Some(position) = IconPosition::from_token(&token) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_icon_position(position);
                Ok(())
            }
            "shortcut" => {
                match value {
                    CapabilityValue::Null => self.set_shortcut(None),
                    other => {
                        let token = expect_string(other)?;
                        let Some(shortcut) = crate::shortcut::Shortcut::from_string(&token) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        self.set_shortcut(Some(shortcut));
                    }
                }
                Ok(())
            }
            "auto_default" => {
                self.set_auto_default(expect_bool(value)?);
                Ok(())
            }
            "auto_exclusive" => {
                self.set_auto_exclusive(expect_bool(value)?);
                Ok(())
            }
            "flat" => {
                self.set_flat(expect_bool(value)?);
                Ok(())
            }
            "elide" => {
                let token = expect_string(value)?;
                let Some(mode) = TextElideMode::from_name(&token) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_elide_mode(mode);
                Ok(())
            }
            "auto_repeat" => {
                self.set_auto_repeat(expect_bool(value)?);
                Ok(())
            }
            "auto_repeat_delay" => {
                self.set_auto_repeat_delay(expect_u32(value)?);
                Ok(())
            }
            "auto_repeat_interval" => {
                self.set_auto_repeat_interval(expect_u32(value)?);
                Ok(())
            }
            "drag_enabled" => {
                self.set_drag_enabled(expect_bool(value)?);
                Ok(())
            }
            "accepts_drops" => {
                self.set_accepts_drops(expect_bool(value)?);
                Ok(())
            }
            #[cfg(full_widgets)]
            "menu_open" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            #[cfg(full_widgets)]
            "menu_item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "icon_source" => {
                match value {
                    CapabilityValue::Null => self.set_icon_source(None),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(Some(IconSource::Builtin(name))),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "icon_tint" => {
                match value {
                    CapabilityValue::Null => self.set_icon_tint(None),
                    CapabilityValue::Color(color) => self.set_icon_tint(Some(color)),
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
        // Mirrors `BUTTON_PROPERTIES` for the properties this control owns; the
        // shared four are appended from `BASE_PROPERTY_NAMES` so they are not
        // retyped per control. The menu names exist only on the `full_widgets`
        // surface, which is the only build that has a menu model to attach.
        #[cfg(full_widgets)]
        return property_names_of![
            "text",
            "pressed",
            "default",
            "icon_source",
            "icon_tint",
            "icon_size",
            "icon_position",
            "shortcut",
            "auto_default",
            "auto_exclusive",
            "flat",
            "elide",
            "drag_enabled",
            "accepts_drops",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            "menu_open",
            "menu_item_count",
            BASE_PROPERTY_NAMES
        ];
        #[cfg(not(full_widgets))]
        return property_names_of![
            "text",
            "pressed",
            "default",
            "icon_source",
            "icon_tint",
            "icon_size",
            "icon_position",
            "shortcut",
            "auto_default",
            "auto_exclusive",
            "flat",
            "elide",
            "drag_enabled",
            "accepts_drops",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            BASE_PROPERTY_NAMES
        ];
    }

    /// Runs one of the commands `button` publishes.
    ///
    /// `click` is the action a programmatic invocation means: press, release and
    /// emit `clicked`, the same sequence a pointer activation produces. It is
    /// deliberately *not* just `press()` — a caller asking for a click expects the
    /// resulting signal, and `press()` alone only mutates the visual state and emits
    /// nothing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "click" => {
                // Route through the public activation, not through `press`/`release`: both are
                // inert on a disabled button, so the old arm emitted `clicked` and notified the
                // group for a control that could not be clicked. `click` also claims the
                // auto-exclusive group selection, which a programmatic activation must do for
                // the same reason a keyboard one does.
                self.click();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl EventHandler for Button {
    /// The four-segment activation contract.
    ///
    /// # Why `released` and `clicked` are not the same event
    ///
    /// A pointer that goes down on a button and comes up away from it is a *canceled*
    /// gesture: the user changed their mind or was dragging something. The rule
    /// is that `clicked` is emitted only when the release lands **inside** the control and
    /// a cancellation is reported otherwise.
    ///
    /// This handler used to treat every release as an activation, so dragging off a
    /// button and letting go still fired it — including for a drag that started
    /// elsewhere and merely ended here.
    ///
    /// | event | result |
    /// |---|---|
    /// | press | `pressed` armed, `pressed` signal emitted |
    /// | move | `pressed` follows whether the pointer is still inside |
    /// | release inside | release, `clicked` |
    /// | release outside | release, `canceled`, **no** `clicked` |
    /// | ungrab (leave / focus loss / disable) | release, `canceled` |
    ///
    /// The whole table applies to a stylus as well as to a mouse (D09-POINTER-01):
    /// `PointerPress`/`PointerMove`/`PointerRelease` take the same arms as their mouse
    /// counterparts, so a pen tap activates the button and a pen drag-off cancels it.
    fn handle_event(&mut self, event: &Event) {
        // Refresh the exclusive group's geometry/enabled snapshot before anything reads it.
        self.sync_group_registration();
        // An open menu owns the next interaction: a click either activates a row or dismisses the
        // popup, and the keyboard drives the highlight instead of the button's activation.
        #[cfg(full_widgets)]
        if self.menu_open {
            let mut consumed = false;
            match event {
                Event::KeyPress { key, .. } if *key == key_codes::ESCAPE => {
                    self.close_menu();
                    consumed = true;
                }
                Event::KeyPress { key, .. } if *key == key_codes::DOWN => {
                    self.move_menu_highlight(true);
                    consumed = true;
                }
                Event::KeyPress { key, .. } if *key == key_codes::UP => {
                    self.move_menu_highlight(false);
                    consumed = true;
                }
                Event::KeyPress { key, .. }
                    if *key == key_codes::ENTER || *key == key_codes::SPACE =>
                {
                    if let Some(index) = self.active_menu_index {
                        self.activate_menu_row(index);
                    }
                    consumed = true;
                }
                Event::MousePress { pos, button, .. } if *button == 1 => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        self.activate_menu_row(index);
                    } else if !self.menu_rect().contains_point(*pos) {
                        self.close_menu();
                    }
                    consumed = true;
                }
                Event::MouseMove { pos } => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        if self.active_menu_index != Some(index) {
                            self.active_menu_index = Some(index);
                            self.base.request_redraw();
                        }
                    }
                }
                _ => { /* Other events still reach the ordinary arms below */ }
            }
            if consumed {
                self.base.handle_event(event);
                return;
            }
        }
        // Gesture logic runs **before** the base records the event. The base both paints and
        // records from the same primitive facts, and it clears `grabbed`/`pressed` on the
        // release — so a control that read them after delegating could no longer tell
        // whether the release belonged to its own gesture. Handling first keeps the arm's
        // guard meaningful, and the base then records the same outcome from the same facts.
        match event {
            Event::MousePress { pos, button, .. } | Event::PointerPress { pos, button, .. }
                if self.base.is_enabled() =>
            {
                // Only a press that resolves to this control arms it. The runtime
                // hit-tests before delivery, but a direct dispatch (a test, a host with
                // its own routing, a designer preview) does not, and an unguarded press
                // would leave the latch armed for a release that belongs elsewhere.
                //
                // A stylus press (`PointerPress`) is a full press here (D09-POINTER-01):
                // before this, a pen tap never armed the button, so it could not activate
                // even though `PointerRelease` already reached the release arm below.
                if button_activates(*button) && self.base.contains_point_with_touch_expansion(*pos)
                {
                    self.press();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.press();
            }
            // `pressed` is a *continuous* quantity, not an edge: dragging off the
            // control clears it and dragging back restores it, so the button visually
            // commits again. The move handling does this.
            // Without it, a press that wandered off
            // left the button painted pressed forever.
            //
            // The arm is guarded on the **grab**, not on `pressed`: guarding on
            // `pressed` made the move that should restore the state exit early, so a
            // drag out and back could never re-arm.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.base.is_grabbed() => {
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            Event::MouseRelease { pos, button, .. } | Event::PointerRelease { pos, button, .. }
                if self.base.is_grabbed() =>
            {
                if !button_activates(*button) {
                    return;
                }
                // Inside ⇒ activate; outside ⇒ cancel. This is the whole contract.
                //
                // A stylus release completes the gesture exactly like a mouse release
                // (D09-POINTER-01), so a pen tap activates the button when it lands inside
                // and cancels when it is dragged off.
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.claim_pointer_activation();
                    self.emit_clicked();
                    #[cfg(full_widgets)]
                    if !self.menu_items.is_empty() {
                        self.open_menu();
                    }
                } else {
                    self.canceled.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } if self.base.is_grabbed() => {
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.claim_pointer_activation();
                    self.emit_clicked();
                    #[cfg(full_widgets)]
                    if !self.menu_items.is_empty() {
                        self.open_menu();
                    }
                } else {
                    self.canceled.emit();
                }
            }
            // D09-EVT-02: a withdrawn contact ends the gesture without activating. The
            // platform can normalise a `TouchCancel` to a `TouchEnd`, so the cancel arrives
            // here as its own internal event; reusing `cancel_gesture` gives exactly the
            // "interaction is off" handling (clear the latch and grab, emit `canceled`) and
            // never emits `clicked`.
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.cancel_gesture();
                self.base.request_redraw();
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if self.base.is_enabled() => {
                // A tap carries no position: the platform already resolved it to this
                // control, which is the same basis the hit test narrows.
                self.emit_clicked();
                self.state_changed.emit(self.state());
            }
            Event::FocusGained { reason } => {
                let _ = reason;
                // `autoDefault`: the button the keyboard is on is the one Enter presses.
                if self.auto_default && self.base.is_enabled() {
                    self.set_default(true);
                }
                self.base.request_redraw();
            }
            Event::FocusLost => {
                // Focus lost mid-press abandons the gesture: the release belongs to
                // whatever the window switch moved to.
                self.cancel_gesture();
                if self.auto_default {
                    self.set_default(false);
                }
                self.base.request_redraw();
            }
            // A declared shortcut activates the button wherever the host delivers it.
            Event::KeyPress { key, modifiers }
                if self.shortcut_matches(*key, *modifiers) && self.base.is_enabled() =>
            {
                self.click();
            }
            // Keyboard activation is reachable only while the button holds focus: a button
            // embedded in a page must not consume Enter or Space from the user's editing.
            Event::KeyPress { key, .. }
                if is_keyboard_activation(*key)
                    && self.base.is_enabled()
                    && self.base.focus_reason().is_some() =>
            {
                // A keyboard activation is `Shortcut`: the user is on the keyboard, so
                // the ring stays lit. Only the activation itself runs here; the ring
                // follows from the focus reason already stored.
                #[cfg(full_widgets)]
                if !self.menu_items.is_empty() {
                    self.toggle_menu();
                } else {
                    self.click();
                }
                #[cfg(not(full_widgets))]
                self.click();
            }
            Event::MouseEnter { .. } => {
                self.request_frame_repaint(None);
            }
            Event::MouseLeave { .. } => {
                // A pointer that leaves while held abandons the painted pressed state,
                // but the *grab* is kept: `MouseLeave` fires as soon as the pointer
                // crosses the edge, and a drag that comes back should still be able to
                // complete. Only the release, wherever it lands, ends the gesture and
                // decides the outcome.
                self.set_pressed(false);
                self.request_frame_repaint(None);
            }
            _ => { /* Other events are not relevant */ }
        }
        // After the control's own gesture: let the base record the primitive facts
        // (hover, press, grab, focus reason) for the state channel.
        self.base.handle_event(event);
    }
}

/// A dropped button leaves its exclusive group.
///
/// The group registry is thread-local and has no lifetime of its own, so a control that is never
/// unregistered would keep a group alive after it is gone.
impl Drop for Button {
    fn drop(&mut self) {
        // Detaches the control from every group it might belong to: the auto-exclusive group its
        // properties name and the group a `ButtonGroup` handle adopted it into.
        button_group::leave_all(self.id());
    }
}

/// A draggable button carries its own caption.
///
/// `text/plain` is the type the crate's text controls already exchange (see `LineEdit`), so a
/// button dragged onto a field needs no adapter at either end.
impl crate::event::dnd::DragSource for Button {
    fn drag_payload(&self, pos: Point) -> Option<crate::event::dnd::DragPayload> {
        if !self.base.is_enabled() || !self.drag_enabled || self.text.is_empty() {
            return None;
        }
        if !self.base.contains_point_with_touch_expansion(pos) {
            return None;
        }
        Some(
            crate::event::dnd::DragPayload::new("text/plain", self.text.clone())
                .with_label(self.text.clone())
                .with_origin(pos),
        )
    }
}

/// A drop-accepting button takes a plain-text payload as its caption.
///
/// `Copy` rather than `Move`: the payload is text, and the source control's own copy is still
/// there — the button is a second place it can appear, not a container that consumed it.
impl crate::event::dnd::DropTarget for Button {
    fn can_accept(&self, payload: &crate::event::dnd::DragPayload) -> bool {
        payload.is_type("text/plain")
    }

    fn on_drop(
        &mut self,
        payload: &crate::event::dnd::DragPayload,
        _pos: Point,
    ) -> crate::event::dnd::DropEffect {
        let label = if payload.label.is_empty() { &payload.item_id } else { &payload.label };
        self.set_text(label.clone());
        crate::event::dnd::DropEffect::Copy
    }

    fn preview_rect(&self, _payload: &crate::event::dnd::DragPayload, _pos: Point) -> Option<Rect> {
        Some(self.geometry())
    }

    fn report_hover(&mut self, _payload: &crate::event::dnd::DragPayload) {
        if !self.drop_hovered {
            self.drop_hovered = true;
            self.base.request_redraw();
        }
    }

    fn report_leave(&mut self) {
        if self.drop_hovered {
            self.drop_hovered = false;
            self.base.request_redraw();
        }
    }
}

/// Whether `button` is one that activates a control.
///
/// Only the primary button activates. The handler used to accept any button, so a
/// secondary click fired a button's action — and a secondary click is how a host opens
/// a context menu (`runtime.rs` routes `mouse_button::SECONDARY` for exactly that).
/// `CheckBox` and `RadioButton` already guarded on the primary button; this brings
/// `Button` into line rather than being the outlier.
pub(crate) fn button_activates(button: u32) -> bool {
    button == crate::event::mouse_button::PRIMARY
}

/// Whether `key` activates a focused button.
///
/// Space and Enter are the two the platform convention assigns, and they are named
/// here rather than left as literals at the match arm: the raw numbers appeared in
/// every binary control with slightly different spellings (`13` here, `13 | 10`
/// elsewhere), so "which keys activate a button" had no single answer to read.
pub(crate) fn is_keyboard_activation(key: u32) -> bool {
    matches!(key, 13 | 10 | 32)
}
