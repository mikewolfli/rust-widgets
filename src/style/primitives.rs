// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::Gradient;
use crate::core::{Color, Font, Size};

/// Whether the user prefers reduced motion (BLUE11 R7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReducedMotionPreference {
    /// The user has expressed no preference; animations run normally.
    #[default]
    NoPreference,
    /// The user asked the OS to reduce motion; callers should suppress or
    /// shorten non-essential animation.
    ReduceMotion,
}

/// Per-side spacing values for padding and margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeOffsets {
    /// Top spacing.
    pub top: u32,
    /// Right spacing.
    pub right: u32,
    /// Bottom spacing.
    pub bottom: u32,
    /// Left spacing.
    pub left: u32,
}

impl EdgeOffsets {
    /// Creates per-side spacing values.
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self { top, right, bottom, left }
    }
    /// Creates equal spacing on all sides.
    pub const fn all(value: u32) -> Self {
        Self::new(value, value, value, value)
    }
    /// Creates symmetric spacing as `(vertical, horizontal)`.
    pub const fn symmetric(vertical: u32, horizontal: u32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
    /// Creates spacing from possibly-negative values, clamping each side to `>= 0`.
    pub fn normalized(top: i32, right: i32, bottom: i32, left: i32) -> Self {
        Self::new(
            normalize_side(top),
            normalize_side(right),
            normalize_side(bottom),
            normalize_side(left),
        )
    }
    /// Returns self as a `Padding` value (identity conversion).
    pub const fn to_padding(&self) -> Padding {
        *self
    }

    /// The total horizontal spacing: left plus right.
    ///
    /// The arithmetic every caller of an `EdgeOffsets` writes when it asks "how much width do
    /// these sides take?" — `implicit_size` adds it to a content width, `content_box`
    /// subtracts it from a rectangle's. Naming it once keeps the two from disagreeing about
    /// whether a padding is applied to one side or both.
    pub const fn horizontal_total(&self) -> u32 {
        self.left.saturating_add(self.right)
    }

    /// The total vertical spacing: top plus bottom.
    pub const fn vertical_total(&self) -> u32 {
        self.top.saturating_add(self.bottom)
    }

    /// The same spacing with its horizontal sides exchanged, for a right-to-left layout.
    ///
    /// Only the horizontal sides move: a mirrored padding must not also swap its top and
    /// bottom, which would lift a control off its own baseline.
    pub const fn mirrored(&self) -> Self {
        Self::new(self.top, self.left, self.bottom, self.right)
    }

    /// `self` with `other`'s sides added to each of its own.
    ///
    /// Used to compose a control's own padding with the padding a container puts around it,
    /// so a nested control's content box is derived from one accumulated value rather than
    /// from two additions performed at the call site.
    pub const fn grow(&self, other: Self) -> Self {
        Self::new(
            self.top.saturating_add(other.top),
            self.right.saturating_add(other.right),
            self.bottom.saturating_add(other.bottom),
            self.left.saturating_add(other.left),
        )
    }
}

/// Inner content spacing. Alias for EdgeOffsets for semantic clarity.
pub type Padding = EdgeOffsets;

/// A padding written one layer at a time, most general first.
///
/// # The four levels
///
/// A caller almost never wants to state all four sides when they all agree. Qt Quick
/// resolves this with `padding` → `horizontalPadding`/`verticalPadding` →
/// `leftPadding`/`rightPadding`/`topPadding`/`bottomPadding`, each more specific layer
/// overriding the one below it:
///
/// ```text
/// leftPadding   <-  horizontalPadding  <-  padding
/// ```
///
/// So `set_uniform(8).set_horizontal(12)` keeps `top`/`bottom` at 8 and moves both
/// horizontal sides to 12, and a later `set_left(20)` moves only the left. That is what
/// makes "this one control has an extra 4 px on the left because of its icon" a one-line
/// statement rather than a re-statement of the other three sides.
///
/// # Why the layers are recorded instead of applied immediately
///
/// `EdgeOffsets` holds four absolute values, so it cannot answer "what would you be, if
/// the horizontal layer were 12?". Keeping the layers separate means the more specific
/// one can be set *before* the general one and still win — which is what lets a style be
/// built in any order, and what a CSS-like cascade requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaddingSpec {
    /// The least specific layer: all four sides.
    uniform: Option<u32>,
    /// The vertical pair.
    vertical: Option<u32>,
    /// The horizontal pair.
    horizontal: Option<u32>,
    /// The most specific layer: one side at a time.
    top: Option<u32>,
    right: Option<u32>,
    bottom: Option<u32>,
    left: Option<u32>,
}

impl PaddingSpec {
    /// An empty spec: every side falls through to 0.
    pub const fn new() -> Self {
        Self {
            uniform: None,
            vertical: None,
            horizontal: None,
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }

    /// The least specific layer: all four sides at once.
    pub const fn set_uniform(mut self, value: u32) -> Self {
        self.uniform = Some(value);
        self
    }

    /// The axis layer: both vertical sides.
    pub const fn set_vertical(mut self, value: u32) -> Self {
        self.vertical = Some(value);
        self
    }

    /// The axis layer: both horizontal sides.
    pub const fn set_horizontal(mut self, value: u32) -> Self {
        self.horizontal = Some(value);
        self
    }

    /// Alias for [`PaddingSpec::set_uniform`], so the builder reads as the CSS cascade
    /// does (`padding(8).horizontal(12).left(20)`).
    pub const fn padding(self, value: u32) -> Self {
        self.set_uniform(value)
    }

    /// Alias for [`PaddingSpec::set_horizontal`].
    pub const fn horizontal(self, value: u32) -> Self {
        self.set_horizontal(value)
    }

    /// Alias for [`PaddingSpec::set_vertical`].
    pub const fn vertical(self, value: u32) -> Self {
        self.set_vertical(value)
    }

    /// One side.
    pub const fn left(mut self, value: u32) -> Self {
        self.left = Some(value);
        self
    }

    /// One side.
    pub const fn right(mut self, value: u32) -> Self {
        self.right = Some(value);
        self
    }

    /// One side.
    pub const fn top(mut self, value: u32) -> Self {
        self.top = Some(value);
        self
    }

    /// One side.
    pub const fn bottom(mut self, value: u32) -> Self {
        self.bottom = Some(value);
        self
    }

    /// Resolves the cascade to concrete per-side values.
    ///
    /// Each side takes the most specific layer that was set, falling back through
    /// `side -> axis -> uniform -> 0`.
    pub const fn resolve(&self) -> EdgeOffsets {
        let base = match self.uniform {
            Some(value) => value,
            None => 0,
        };
        let vertical = match self.vertical {
            Some(value) => value,
            None => base,
        };
        let horizontal = match self.horizontal {
            Some(value) => value,
            None => base,
        };
        EdgeOffsets {
            top: match self.top {
                Some(value) => value,
                None => vertical,
            },
            right: match self.right {
                Some(value) => value,
                None => horizontal,
            },
            bottom: match self.bottom {
                Some(value) => value,
                None => vertical,
            },
            left: match self.left {
                Some(value) => value,
                None => horizontal,
            },
        }
    }

    /// Whether every layer is unset, so the spec adds nothing.
    pub const fn is_empty(&self) -> bool {
        self.uniform.is_none()
            && self.vertical.is_none()
            && self.horizontal.is_none()
            && self.top.is_none()
            && self.right.is_none()
            && self.bottom.is_none()
            && self.left.is_none()
    }
}
/// Outer widget margin. Alias for EdgeOffsets for semantic clarity.
pub type Margin = EdgeOffsets;

impl Default for EdgeOffsets {
    fn default() -> Self {
        Self::all(0)
    }
}

const fn normalize_side(value: i32) -> u32 {
    if value <= 0 {
        0
    } else {
        value as u32
    }
}

/// Drop-shadow style token.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Horizontal offset.
    pub x: i32,
    /// Vertical offset.
    pub y: i32,
    /// Blur radius.
    pub blur: u32,
    /// Shadow color.
    pub color: Color,
}

impl Shadow {
    /// Creates a new default shadow.
    pub fn new() -> Self {
        Self { x: 0, y: 0, blur: 0, color: Color::BLACK }
    }
    /// Sets the shadow offset.
    pub fn with_offset(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }
    /// Sets the shadow blur radius.
    pub fn with_blur(mut self, blur: u32) -> Self {
        self.blur = blur;
        self
    }
    /// Sets the shadow color.
    pub fn with_color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
}

crate::impl_default_via_new!(Shadow);

/// Minimum touch target dimensions by device class (BLUE8 P4-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchTargetSize {
    /// PC touch screen: 32×32 pt.
    Desktop,
    /// Tablet: 44×44 pt.
    Tablet,
    /// Phone: 48×48 pt.
    Phone,
    /// Embedded: 40×40 pt.
    Embedded,
    /// Projection/remote control: 24×24 pt. (gated behind `projection` feature).
    #[cfg(feature = "projection")]
    Projection,
}

impl TouchTargetSize {
    /// Returns the minimum recommended pixel dimensions for this class.
    pub const fn dimensions(self) -> Size {
        match self {
            Self::Desktop => Size::new(32, 32),
            Self::Tablet => Size::new(44, 44),
            Self::Phone => Size::new(48, 48),
            Self::Embedded => Size::new(40, 40),
            #[cfg(feature = "projection")]
            Self::Projection => Size::new(24, 24),
        }
    }

    /// Returns the recommended spacing between interactive elements.
    pub const fn spacing(self) -> u32 {
        match self {
            Self::Desktop => 8,
            Self::Tablet => 12,
            Self::Phone => 16,
            Self::Embedded => 10,
            #[cfg(feature = "projection")]
            Self::Projection => 6,
        }
    }
}

/// Resolved style values applied to a widget.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetStyle {
    /// Optional background color.
    pub background_color: Option<Color>,
    /// Optional background gradient.
    pub background_gradient: Option<Gradient>,
    /// Optional text color.
    pub text_color: Option<Color>,
    /// Optional text font.
    pub font: Option<Font>,
    /// Optional border color.
    pub border_color: Option<Color>,
    /// Border width in logical pixels. `None` = inherit from parent.
    pub border_width: Option<u32>,
    /// Border radius in logical pixels. `None` = inherit from parent.
    pub border_radius: Option<u32>,
    /// Inner content padding.
    pub padding: Padding,
    /// Outer widget margin.
    pub margin: Margin,
    /// The gap between a control's **indicator and its label**.
    ///
    /// # Why spacing is not "the gap between siblings"
    ///
    /// QML's `CheckBox.qml:61` and `ComboBox.qml:21` both set `spacing`, and in both it
    /// means exactly one thing: the distance from the control's own indicator to its own
    /// text. It is a fact about *this control's* contents, so a checkbox, a radio and a
    /// menu item with the same `spacing` look like one family.
    ///
    /// The gap between two *siblings* is a different fact and belongs to the layout that
    /// places them (`FlexLayout::gap`), because the sibling pair is the layout's
    /// knowledge, not either control's. Mixing the two is why this crate had the same
    /// number written into both roles and neither could be changed safely.
    ///
    /// `None` means the control uses its own default from the metrics table.
    pub spacing: Option<u32>,
    /// Optional drop shadow.
    pub shadow: Option<Shadow>,
    /// Optional minimum touch-target size override (BLUE8 P4-4).
    /// When set, hit testing expands the effective area to this size.
    pub touch_target: Option<Size>,
    /// Optional opacity (0.0 = transparent, 1.0 = opaque). Set via CSS `opacity`.
    pub opacity: Option<f32>,
    /// Records that this style is currently **the active theme's rendering of this
    /// control**, so re-applying a theme may replace its colours.
    ///
    /// # Why provenance has to be recorded
    ///
    /// [`merge`](Self::merge) fills only fields that are `None`, which is what makes a
    /// caller-set colour survive the theme. That rule alone cannot express "replace what
    /// the *previous* theme put here", so re-applying a theme after a switch left every
    /// already-styled control on the old palette: the fields were no longer `None`, so
    /// the new theme had nowhere to write. Switching light → dark kept light windows,
    /// which is a visible defect, not a subtlety.
    ///
    /// The flag says "the theme is the author of this style". A caller that sets any
    /// style property clears it (see [`with_background`](Self::with_background) and
    /// friends), because at that point the caller is an author too and the theme must
    /// fall back to fill-only-`None` to avoid overwriting them.
    ///
    /// Not part of the visual contract: it is never rendered.
    pub theme_derived: bool,
}

impl WidgetStyle {
    /// Merges `other` as a **theme** would.
    ///
    /// Two cases, and the flag is what separates them:
    ///
    /// * **`theme_derived`** — the theme authored this style, so its values are replaced
    ///   wholesale. This is the case a second theme application needs, and the case
    ///   [`merge`](Self::merge) cannot express.
    /// * **otherwise** — the caller has set something, so fall back to `merge`'s
    ///   fill-only-`None` rule and never overwrite them. Caller precedence wins.
    ///
    /// Either way the result is marked theme-derived *only* if nothing but the theme is
    /// in it; a style that also carries caller values keeps the flag clear so the next
    /// application cannot silently replace them.
    pub fn merge_theme(&mut self, other: &WidgetStyle) {
        if self.theme_derived {
            // The theme is the sole author: replace its previous rendering, keeping only
            // the geometry-adjacent fields it does not speak to.
            self.background_color = other.background_color;
            self.background_gradient = other.background_gradient.clone();
            self.text_color = other.text_color;
            self.font = other.font.clone();
            self.border_color = other.border_color;
            self.border_width = other.border_width;
            self.border_radius = other.border_radius;
            self.shadow = other.shadow.clone();
            self.touch_target = other.touch_target;
            self.opacity = other.opacity;
            self.theme_derived = true;
        } else {
            self.merge(other);
            // `merge` fills only `None`, so anything it actually wrote is the theme's,
            // while anything already set belongs to the caller. Marking the style as
            // theme-derived here would let the next application overwrite caller values,
            // so the flag stays clear.
        }
    }
    /// Sets the background color.
    ///
    /// Setting any style property makes the caller a co-author, so the theme-derived
    /// mark is cleared: from here on the theme may only fill fields that are `None`, and
    /// must not replace this colour on a later theme application.
    pub fn with_background(mut self, c: Color) -> Self {
        self.background_color = Some(c);
        self.theme_derived = false;
        self
    }
    /// Sets the text color.
    pub fn with_text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self.theme_derived = false;
        self
    }
    /// Sets the font.
    pub fn with_font(mut self, f: Font) -> Self {
        self.font = Some(f);
        self.theme_derived = false;
        self
    }
    /// Sets the border.
    pub fn with_border(mut self, color: Color, width: u32, radius: u32) -> Self {
        self.border_color = Some(color);
        self.border_width = Some(width);
        self.border_radius = Some(radius);
        self.theme_derived = false;
        self
    }
    /// Sets the padding.
    pub fn with_padding(mut self, p: Padding) -> Self {
        self.padding = p;
        self
    }
    /// Sets the padding from a four-level cascade.
    ///
    /// The builder counterpart of [`PaddingSpec`]: `with_padding_spec(PaddingSpec::new()
    /// .padding(8).horizontal(12))` states "8 all round, 12 on the sides" without having
    /// to re-state the two sides that did not change.
    pub fn with_padding_spec(mut self, spec: PaddingSpec) -> Self {
        self.padding = spec.resolve();
        self
    }
    /// Sets the gap between this control's indicator and its label.
    ///
    /// See [`WidgetStyle::spacing`] for why this is not a sibling layout parameter.
    pub fn with_spacing(mut self, spacing: u32) -> Self {
        self.spacing = Some(spacing);
        self.theme_derived = false;
        self
    }
    /// Sets the margin.
    pub fn with_margin(mut self, m: Margin) -> Self {
        self.margin = m;
        self
    }
    /// Sets the shadow.
    pub fn with_shadow(mut self, s: Shadow) -> Self {
        self.shadow = Some(s);
        self
    }
    /// Sets the minimum touch target size (BLUE8 P4-4).
    pub fn with_touch_target(mut self, target: Size) -> Self {
        self.touch_target = Some(target);
        self
    }
    /// Sets the background gradient.
    pub fn with_gradient(mut self, g: Gradient) -> Self {
        self.background_gradient = Some(g);
        self
    }
    /// Sets the opacity (CSS `opacity`).
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }

    /// Create a child style by inheriting non-set properties from a parent style.
    /// Properties that are `None` in `self` fall back to the parent's values.
    pub fn inherit_from(&self, parent: &WidgetStyle) -> WidgetStyle {
        WidgetStyle {
            background_color: self.background_color.or(parent.background_color),
            background_gradient: self
                .background_gradient
                .clone()
                .or(parent.background_gradient.clone()),
            text_color: self.text_color.or(parent.text_color),
            font: self.font.clone().or(parent.font.clone()),
            border_color: self.border_color.or(parent.border_color),
            border_width: self.border_width.or(parent.border_width),
            border_radius: self.border_radius.or(parent.border_radius),
            padding: self.padding,
            margin: self.margin,
            // Like `padding`, a control's indicator-to-label gap is a fact about the
            // control itself, so it does not inherit: a child with its own spacing keeps
            // it, and a child without one takes the metrics default rather than the
            // parent's value.
            spacing: self.spacing,
            shadow: self.shadow.clone().or(parent.shadow.clone()),
            touch_target: self.touch_target.or(parent.touch_target),
            opacity: self.opacity.or(parent.opacity),
            // Inheriting from a parent does not make a style the theme's: the child's
            // authorship is decided by the theme application that runs next.
            theme_derived: false,
        }
    }

    /// Merge another style into this one: set each property if it's `None` (or default).
    pub fn merge(&mut self, other: &WidgetStyle) {
        if self.background_color.is_none() {
            self.background_color = other.background_color;
        }
        if self.background_gradient.is_none() {
            self.background_gradient.clone_from(&other.background_gradient);
        }
        if self.text_color.is_none() {
            self.text_color = other.text_color;
        }
        if self.font.is_none() {
            self.font.clone_from(&other.font);
        }
        if self.border_color.is_none() {
            self.border_color = other.border_color;
        }
        if self.border_width.is_none() {
            self.border_width = other.border_width;
        }
        if self.border_radius.is_none() {
            self.border_radius = other.border_radius;
        }
        if self.shadow.is_none() {
            self.shadow = other.shadow.clone();
        }
        if self.touch_target.is_none() {
            self.touch_target = other.touch_target;
        }
        if self.opacity.is_none() {
            self.opacity = other.opacity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_and_margin_normalize_negative_values() {
        let padding = Padding::normalized(-1, 4, -99, 8);
        let margin = Margin::normalized(-5, 0, 3, 2);
        assert_eq!(padding, Padding::new(0, 4, 0, 8));
        assert_eq!(margin, Margin::new(0, 0, 3, 2));
    }

    #[test]
    fn padding_and_margin_support_symmetric_builders() {
        assert_eq!(Padding::symmetric(6, 2), Padding::new(6, 2, 6, 2));
        assert_eq!(Margin::symmetric(3, 5), Margin::new(3, 5, 3, 5));
    }

    #[test]
    fn inherit_from_falls_back_to_parent() {
        let parent =
            WidgetStyle::default().with_background(Color::RED).with_text_color(Color::BLUE);
        let child = WidgetStyle::default();
        let inherited = child.inherit_from(&parent);
        assert_eq!(inherited.background_color, Some(Color::RED));
        assert_eq!(inherited.text_color, Some(Color::BLUE));
    }

    #[test]
    fn inherit_from_child_takes_precedence() {
        let parent = WidgetStyle::default().with_background(Color::RED);
        let child = WidgetStyle::default().with_background(Color::GREEN);
        let inherited = child.inherit_from(&parent);
        assert_eq!(inherited.background_color, Some(Color::GREEN));
    }

    #[test]
    fn merge_updates_missing_properties() {
        let mut base = WidgetStyle::default().with_background(Color::RED);
        let overlay = WidgetStyle::default().with_text_color(Color::BLUE);
        base.merge(&overlay);
        assert_eq!(base.background_color, Some(Color::RED));
        assert_eq!(base.text_color, Some(Color::BLUE));
    }

    #[test]
    fn merge_does_not_override_existing() {
        let mut base = WidgetStyle::default().with_background(Color::RED);
        let overlay = WidgetStyle::default().with_background(Color::GREEN);
        base.merge(&overlay);
        assert_eq!(base.background_color, Some(Color::RED)); // unchanged
    }

    // ── The four-level padding cascade (P0-3) ────────────────────────────

    #[test]
    fn each_padding_layer_overrides_only_the_one_below_it() {
        // The case the cascade exists for: state the general rule once, then change one
        // axis. Without layers this call site would have to re-state all four sides.
        let resolved = PaddingSpec::new().padding(8).horizontal(12).resolve();
        assert_eq!(resolved.top, 8, "the vertical pair keeps the general value");
        assert_eq!(resolved.bottom, 8);
        assert_eq!(resolved.left, 12);
        assert_eq!(resolved.right, 12);
    }

    #[test]
    fn a_side_overrides_its_axis_and_the_axis_overrides_the_uniform_layer() {
        let resolved = PaddingSpec::new().padding(4).horizontal(10).left(20).resolve();
        assert_eq!(resolved.left, 20, "the most specific layer wins");
        assert_eq!(resolved.right, 10, "its sibling keeps the axis value");
        assert_eq!(resolved.top, 4, "the untouched axis keeps the general value");
        assert_eq!(resolved.bottom, 4);
    }

    #[test]
    fn writing_a_side_does_not_disturb_the_other_three() {
        // BLUE22 §3 判据: "单测：写 `left_padding` 只改左".
        let before = PaddingSpec::new().padding(6).resolve();
        let after = PaddingSpec::new().padding(6).left(15).resolve();
        assert_eq!(after.left, 15);
        assert_eq!(after.top, before.top);
        assert_eq!(after.right, before.right);
        assert_eq!(after.bottom, before.bottom);
    }

    #[test]
    fn a_more_specific_layer_wins_even_if_it_was_set_first() {
        // The reason the layers are *recorded* rather than applied in order: a style may be
        // built in any sequence, and the more specific statement must still win. Applying
        // eagerly would make `left(20).padding(4)` lose its 20.
        let resolved = PaddingSpec::new().left(20).padding(4).resolve();
        assert_eq!(resolved.left, 20);
        assert_eq!(resolved.right, 4);
    }

    #[test]
    fn an_empty_spec_resolves_to_no_padding() {
        let spec = PaddingSpec::new();
        assert!(spec.is_empty());
        assert_eq!(spec.resolve(), EdgeOffsets::all(0));
        assert!(!PaddingSpec::new().padding(0).is_empty(), "an explicit 0 is still a statement");
    }

    #[test]
    fn the_vertical_and_horizontal_axes_are_independent() {
        let resolved = PaddingSpec::new().vertical(2).horizontal(9).resolve();
        assert_eq!((resolved.top, resolved.bottom), (2, 2));
        assert_eq!((resolved.left, resolved.right), (9, 9));
    }

    #[test]
    fn a_style_can_be_given_a_padding_spec_and_a_spacing() {
        let style = WidgetStyle::default()
            .with_padding_spec(PaddingSpec::new().padding(8).horizontal(12))
            .with_spacing(6);
        assert_eq!(style.padding.top, 8);
        assert_eq!(style.padding.horizontal_total(), 24);
        assert_eq!(style.spacing, Some(6));
    }
}
