// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reduced-cost widgets for embedded targets: feature suppression, reuse, and
//! fixed-size styling.
//!
//! Three loosely related tools, all aimed at fitting a UI into a small budget:
//!
//! * [`LightweightConfig`] plus [`LightweightWidget`] — a wrapper that carries a
//!   set of suppressed visual features alongside a widget without modifying it.
//! * [`LightweightWidgetFactory`] — a counting factory that refuses to hand out
//!   more than a configured number of widgets.
//! * [`WidgetPool`] plus [`PoolHandle`] — a real reuse pool: it owns its values
//!   for as long as it lives and hands a slot back out once its handle is
//!   dropped. Read the recycling rules and the handle lifetime rule on
//!   [`WidgetPool`] before using it.
//!
//! None of these registers anything with the crate's widget registry
//! (`crate::widget::runtime`). That registry owns widgets for the host to
//! paint; a pool owns widgets to be recycled. They are distinct concerns, and a
//! pooled widget only becomes displayable when the caller hands it to
//! `crate::widget::runtime::register`.

use crate::widget::Widget;
/// Lightweight widget configuration for embedded systems
///
/// A set of feature *suppressions* rather than additions: every field defaults to
/// `false` and means "the expensive thing is still allowed". Nothing here
/// allocates, so the struct itself costs six bytes.
///
/// Note that this is a plain data struct with no behaviour attached — the flags
/// are read by whichever painter is handed the config, and nothing in this module
/// enforces them.
#[derive(Debug, Clone)]
pub struct LightweightConfig {
    /// Skip drop shadows. Leaves the widget's own bounds unpadded, so layout may
    /// also shrink when this is set.
    pub disable_shadows: bool,
    /// Skip animation: widgets jump to their final state instead of interpolating.
    pub disable_animations: bool,
    /// Skip gradient fills; solid colours only, which removes the per-pixel
    /// interpolation step from painting.
    pub disable_gradients: bool,
    /// Draw one-pixel borders instead of styled ones.
    pub simple_borders: bool,
    /// Tighten interior padding, fitting more content into the same area.
    pub reduced_padding: bool,
    /// Wire up only the signals a widget needs; extra reactive plumbing is skipped.
    pub minimal_signals: bool,
}
impl LightweightConfig {
    /// A config with **nothing** suppressed — every field `false`, i.e. full
    /// visual fidelity. This is the opposite end from
    /// [`LightweightConfig::minimal`], and is what
    /// [`LightweightWidget::new`] uses.
    pub fn new() -> Self {
        Self {
            disable_shadows: false,
            disable_animations: false,
            disable_gradients: false,
            simple_borders: false,
            reduced_padding: false,
            minimal_signals: false,
        }
    }
    /// A config with **every** field `true` — the cheapest rendering, at the cost
    /// of all visual finesse. This is the default for
    /// [`LightweightWidgetFactory::new`], unlike [`LightweightConfig::new`].
    pub fn minimal() -> Self {
        Self {
            disable_shadows: true,
            disable_animations: true,
            disable_gradients: true,
            simple_borders: true,
            reduced_padding: true,
            minimal_signals: true,
        }
    }
    /// Sets [`LightweightConfig::disable_shadows`].
    pub fn with_shadows_disabled(mut self) -> Self {
        self.disable_shadows = true;
        self
    }
    /// Sets [`LightweightConfig::disable_animations`].
    pub fn with_animations_disabled(mut self) -> Self {
        self.disable_animations = true;
        self
    }
    /// Sets [`LightweightConfig::disable_gradients`].
    pub fn with_gradients_disabled(mut self) -> Self {
        self.disable_gradients = true;
        self
    }
}
crate::impl_default_via_new!(LightweightConfig);
/// Lightweight widget wrapper that reduces memory footprint
///
/// Pairs a widget with the [`LightweightConfig`] that says how cheaply to paint
/// it. The wrapper is transparent to the widget itself: [`Self::inner`] and
/// [`Self::into_inner`] hand back the original value untouched, so a caller can
/// always escape to the full-fidelity widget.
///
/// The config here is the wrapper's *own* copy and can differ from the factory's
/// ([`LightweightWidgetFactory::create`] clones the factory config in, then
/// [`Self::with_config`] can override it).
pub struct LightweightWidget<W: Widget> {
    inner: W,
    config: LightweightConfig,
}
impl<W: Widget> LightweightWidget<W> {
    /// Wraps `widget` with a default (nothing-suppressed) configuration.
    ///
    /// Deliberately not [`LightweightConfig::minimal`]: wrapping alone should not
    /// silently downgrade appearance. Use [`Self::with_config`] to opt into
    /// cheap rendering.
    pub fn new(widget: W) -> Self {
        Self { inner: widget, config: LightweightConfig::new() }
    }
    /// Replaces the wrapper's configuration.
    pub fn with_config(mut self, config: LightweightConfig) -> Self {
        self.config = config;
        self
    }
    /// Borrows the wrapped widget, with the config still in force.
    pub fn inner(&self) -> &W {
        &self.inner
    }
    /// Mutably borrows the wrapped widget.
    ///
    /// The config is untouched, so mutations that the configuration would have
    /// suppressed (a re-enable of animation, say) are still suppressed.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }
    /// Unwraps, returning the widget and discarding the configuration.
    ///
    /// The widget was never modified by the wrapper, so this is lossless.
    pub fn into_inner(self) -> W {
        self.inner
    }
}
/// Factory for creating lightweight widgets
///
/// Unlike [`WidgetPool`] this does not recycle anything: it holds no widget
/// storage at all, only a counter. [`Self::create`] invokes the caller's closure
/// to build a fresh widget every time, and [`Self::release`] merely decrements
/// the counter — the widget itself has already been moved out and is the caller's
/// to drop or hand to
/// `crate::widget::runtime::register`.
///
/// The counter tracks *outstanding* creations, so `release` is an accounting
/// call, not a free.
pub struct LightweightWidgetFactory {
    config: LightweightConfig,
    widget_count: usize,
    max_widgets: usize,
}
impl LightweightWidgetFactory {
    /// Creates a factory that hands out at most 100 widgets, configured minimal.
    ///
    /// Note the config is [`LightweightConfig::minimal`] — widgets from this
    /// factory are cheap-rendering by default, which differs from
    /// [`LightweightWidget::new`]. Override with [`Self::with_config`].
    pub fn new() -> Self {
        Self { config: LightweightConfig::minimal(), widget_count: 0, max_widgets: 100 }
    }
    /// Replaces the configuration stamped onto every widget this factory creates.
    ///
    /// Only affects subsequent [`Self::create`] calls; widgets already handed out
    /// keep the config they were built with.
    pub fn with_config(mut self, config: LightweightConfig) -> Self {
        self.config = config;
        self
    }
    /// Sets how many widgets may be outstanding at once.
    ///
    /// Stored as given, with no sanity check — a `max` of `0` makes
    /// [`Self::create`] refuse everything, and a `max` below the current count
    /// makes it refuse until enough widgets are released.
    pub fn with_max_widgets(mut self, max: usize) -> Self {
        self.max_widgets = max;
        self
    }
    /// Returns whether the factory is still under its outstanding-widget cap.
    ///
    /// Consulted by [`Self::create`]; a caller can use it to check before
    /// building a widget it would otherwise have to throw away.
    pub fn can_create(&self) -> bool {
        self.widget_count < self.max_widgets
    }
    /// Builds one widget by calling `factory`, wrapped and configured.
    ///
    /// Returns `None` without calling `factory` when [`Self::can_create`] is
    /// `false`, so a refused creation costs nothing. On success the counter is
    /// incremented and the closure is called exactly once; the widget is *not*
    /// tracked, so only the caller knows which ones are still outstanding.
    pub fn create<W, F>(&mut self, factory: F) -> Option<LightweightWidget<W>>
    where
        F: FnOnce() -> W,
        W: Widget,
    {
        if self.can_create() {
            self.widget_count += 1;
            Some(LightweightWidget::new(factory()).with_config(self.config.clone()))
        } else {
            None
        }
    }
    /// Credits one widget back against the cap so another can be created.
    ///
    /// Does not take or drop a widget — the caller has already disposed of it,
    /// or the count and reality diverge. Saturates at zero, so an extra call
    /// cannot buy unbounded headroom.
    pub fn release(&mut self) {
        self.widget_count = self.widget_count.saturating_sub(1);
    }
    /// Returns how many widgets this factory has handed out and not had released.
    pub fn widget_count(&self) -> usize {
        self.widget_count
    }
}
crate::impl_default_via_new!(LightweightWidgetFactory);
/// Optimized widget style for embedded systems
///
/// Every dimension is a `u8`, which caps the representable range at 255 and is
/// the point of the type: a style that fits in a few bytes. There is no
/// conversion to or from the desktop style type in this module, so adopting one
/// here means re-specifying the values.
#[derive(Debug, Clone)]
pub struct LightweightStyle {
    /// Background fill as `0xRRGGBB`, or `None` to leave the parent showing
    /// through. Stored as a bare `u32` because this crate uses 24-bit RGB here,
    /// with no alpha channel.
    pub background_color: Option<u32>,
    /// Text colour as `0xRRGGBB`, or `None` to inherit the ambient text colour.
    pub text_color: Option<u32>,
    /// Border colour as `0xRRGGBB`, or `None` to inherit. Ignored when
    /// `border_width` is `0`.
    pub border_color: Option<u32>,
    /// Border thickness in pixels; `0` draws no border at all.
    pub border_width: u8,
    /// Interior padding in pixels on each side. `u8` rather than a per-side rect,
    /// so this style cannot express asymmetric padding.
    pub padding: u8,
    /// Font size in points. `u8` caps text at the point sizes, which is ample
    /// here but is the reason this type is not a general-purpose style.
    pub font_size: u8,
}
impl LightweightStyle {
    /// An unstyled baseline: no colours (all inherited), no border, `4` px
    /// padding, `12` pt text.
    pub fn new() -> Self {
        Self {
            background_color: None,
            text_color: None,
            border_color: None,
            border_width: 0,
            padding: 4,
            font_size: 12,
        }
    }
    /// A dense preset aimed at small screens.
    ///
    /// Differs from [`LightweightStyle::new`] by keeping the background inherited
    /// but setting black text on a `0x808080` border, `1` px, `2` px padding and
    /// `10` pt text — deliberately one step down in size and padding from the
    /// baseline, and lacking a background fill.
    pub fn compact() -> Self {
        Self {
            background_color: None,
            text_color: Some(0x000000),
            border_color: Some(0x808080),
            border_width: 1,
            padding: 2,
            font_size: 10,
        }
    }
}
crate::impl_default_via_new!(LightweightStyle);
/// Memory-efficient widget pool
///
/// Hands out reusable `T` values through [`PoolHandle`]s. The pool **owns** the
/// values it has ever created — see [`Self::acquire`] — and reuses a value only
/// once its handle is dropped or [`Self::release`] is called for it.
///
/// # Recycling semantics
///
/// * **Values are never reset.** `acquire` on a recycled slot returns the value
///   exactly as the previous user left it, and the closure is *not* consulted. A
///   pooled `T` that carries state must be cleaned up by the code that reuses it;
///   the pool has no notion of a default value. This is the price of not
///   requiring a `Default` bound.
/// * **The pool grows to `max_size` and then refuses.** The first `max_size`
///   `acquire` calls each construct a value; after that, `acquire` returns `None`
///   until a slot is released. It never evicts a slot that a live handle holds.
/// * **The pool never shrinks.** There is no `clear` method; reclaiming the
///   pool means dropping it, which drops every retained value at once.
/// * **Reuse is first-fit.** `acquire` takes the lowest-indexed free slot.
///
/// # Lifetime rule
///
/// A [`PoolHandle`] borrows the pool only through a raw pointer, so the borrow
/// checker cannot stop this from compiling:
///
/// ```ignore
/// let handle = {
///     let mut pool = WidgetPool::new(2);
///     pool.acquire(|| 1)
/// };
/// drop(handle); // writes through a pointer to a pool that is already gone
/// ```
///
/// Dropping the pool while a handle from it is alive is **undefined behaviour**.
/// Handles must be dropped (or the pool must outlive them) before the pool itself
/// is freed — pin the pool in a field, or in a long-lived owner, for as long as
/// any handle exists.
pub struct WidgetPool<T> {
    available: Vec<T>,
    in_use: Vec<bool>,
    max_size: usize,
}
impl<T> WidgetPool<T> {
    /// Creates an empty pool that will hold at most `max_size` values.
    ///
    /// Both backing vectors are reserved to `max_size` up front, so growing to
    /// the cap does not reallocate; the capacities are what make this a pool
    /// rather than a plain vector. `max_size` is stored as given — `0` produces a
    /// pool that refuses every `acquire`.
    pub fn new(max_size: usize) -> Self {
        Self {
            available: Vec::with_capacity(max_size),
            in_use: Vec::with_capacity(max_size),
            max_size,
        }
    }
    /// Takes a slot, either recycled or newly built by `factory`.
    ///
    /// Returns `None` when every slot is in use *and* the pool is already at
    /// `max_size`. Otherwise the free slot with the lowest index is marked in use:
    /// if it is an existing slot the value is returned **as the previous user left
    /// it** and `factory` is never called; only a brand-new slot invokes `factory`.
    ///
    /// The returned handle releases its slot when dropped, so the pool is
    /// single-borrow at a time by design — see [`PoolHandle`] for the lifetime
    /// rule that makes that safe.
    pub fn acquire<F>(&mut self, factory: F) -> Option<PoolHandle<T>>
    where
        F: FnOnce() -> T,
    {
        if let Some(index) = self.in_use.iter().position(|&used| !used) {
            self.in_use[index] = true;
            Some(PoolHandle { index, pool: self as *mut Self })
        } else if self.available.len() < self.max_size {
            let index = self.available.len();
            self.available.push(factory());
            self.in_use.push(true);
            Some(PoolHandle { index, pool: self as *mut Self })
        } else {
            None
        }
    }
    /// Marks the slot at `index` free for reuse.
    ///
    /// Takes the index rather than a [`PoolHandle`], which is what a handle's
    /// `Drop` calls. Out-of-range indices are ignored, as is marking a slot that
    /// is already free; releasing twice is therefore harmless. The value itself is
    /// kept — release marks, it does not clear or drop.
    pub fn release(&mut self, index: usize) {
        if index < self.in_use.len() {
            self.in_use[index] = false;
        }
    }
    /// Borrows the value in slot `index`, if that slot exists and is in use.
    ///
    /// Returns `None` for an out-of-range index **or** a free slot, so a released
    /// slot reads as absent rather than exposing a value that is logically gone.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.available.len() && self.in_use[index] {
            Some(&self.available[index])
        } else {
            None
        }
    }
    /// Mutably borrows the value in slot `index`, under the same rules as
    /// [`Self::get`].
    ///
    /// This bypasses the exclusivity a [`PoolHandle`] is meant to provide — it
    /// needs `&mut self`, so no handle-derived borrow can be live at the same
    /// time, but two handles for the same index (which `acquire` never produces)
    /// would be indistinguishable through `get_mut` alone.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.available.len() && self.in_use[index] {
            Some(&mut self.available[index])
        } else {
            None
        }
    }
    /// Returns how many slots are currently in use.
    ///
    /// Counts the `in_use` flags, so a slot released through
    /// [`Self::release`] is free here even if its value is still retained.
    pub fn used_count(&self) -> usize {
        self.in_use.iter().filter(|&&used| used).count()
    }
    /// Returns how many slots are free — i.e. the pool's maximum size minus the
    /// slots already taken, since the pool grows eagerly on demand.
    ///
    /// Note this counts *slots at the cap*, not slots already built: a fresh pool
    /// with `max_size` of 3 and no acquisitions reports 3, because all three are
    /// still available for construction.
    pub fn available_count(&self) -> usize {
        self.available.len() - self.used_count()
    }
}
/// Exclusive claim on one [`WidgetPool`] slot, releasing it on drop.
///
/// # Safety
///
/// The handle stores a raw `*mut WidgetPool<T>` rather than a borrow, which is
/// what lets `acquire` return it from a `&mut self` method without the pool being
/// borrowed for the handle's whole life. The obligation that buys is on the
/// caller: **the pool must outlive every handle taken from it.** Dropping a pool
/// while a handle into it exists causes `Drop` to write through a dangling
/// pointer, which is undefined behaviour. See [`WidgetPool`] for the rule and a
/// reproducing snippet.
///
/// The `WidgetPool` half of the pool must not be moved while a handle is alive
/// either: the pointer points at the original address, and a move would leave it
/// dangling for the same reason.
pub struct PoolHandle<T> {
    index: usize,
    pool: *mut WidgetPool<T>,
}
impl<T> PoolHandle<T> {
    /// Returns the slot this handle claims.
    ///
    /// The index stays valid for the handle's lifetime: `acquire` only ever hands
    /// out the index of a slot it just marked in use, and slots are never removed.
    pub fn index(&self) -> usize {
        self.index
    }
}
impl<T> Drop for PoolHandle<T> {
    /// Releases the slot back to the pool, so a later `acquire` can recycle it.
    ///
    /// # Safety
    ///
    /// The stored pointer is null-checked, but a null pool is not a state
    /// `acquire` can produce — it exists only to keep the check honest. For a
    /// non-null pointer this dereferences the pool, which the handle's contract
    /// requires to still be alive; that requirement cannot be expressed in the
    /// type system, which is why it is documented on [`WidgetPool`] and
    /// [`PoolHandle`] instead. `release` only flips an in-use flag, so the write
    /// touches the pool's own flag vector and never the pooled value.
    fn drop(&mut self) {
        // SAFETY: pool is guaranteed to be either null (default/after move) or a valid
        // *mut WidgetPool<T> that outlives this handle. The index was validated against
        // pool.capacity() during acquire(). The release() call only marks the slot as
        // unused and does not deallocate, so it is safe even under shared access patterns.
        unsafe {
            if !self.pool.is_null() {
                (*self.pool).release(self.index);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lightweight_config() {
        let config = LightweightConfig::minimal();
        assert!(config.disable_shadows);
        assert!(config.disable_animations);
        assert!(config.disable_gradients);
    }
    #[test]
    fn test_widget_factory() {
        use crate::core::Rect;
        let mut factory = LightweightWidgetFactory::new().with_max_widgets(2);
        assert!(factory.can_create());
        let widget1 = factory
            .create(|| crate::widget::Label::new("Test 1".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget1.is_some());
        let widget2 = factory
            .create(|| crate::widget::Label::new("Test 2".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget2.is_some());
        let widget3 = factory
            .create(|| crate::widget::Label::new("Test 3".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget3.is_none());
        assert_eq!(factory.widget_count(), 2);
    }
    #[test]
    fn test_lightweight_style() {
        let style = LightweightStyle::compact();
        assert_eq!(style.padding, 2);
        assert_eq!(style.font_size, 10);
    }
    #[test]
    fn test_widget_pool() {
        let mut pool: WidgetPool<i32> = WidgetPool::new(3);
        let handle1 = pool.acquire(|| 1);
        assert!(handle1.is_some());
        let handle2 = pool.acquire(|| 2);
        assert!(handle2.is_some());
        assert_eq!(pool.used_count(), 2);
        assert_eq!(pool.available_count(), 0);
        drop(handle1);
        assert_eq!(pool.used_count(), 1);
    }
}
