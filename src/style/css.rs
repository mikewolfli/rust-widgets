// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CSS stylesheet parser — parses CSS text into rules with property declarations
//! that can be matched against widgets and applied to `WidgetStyle`.
//!
//! # Example
//! ```rust
//! use rust_widgets::style::css::CssParser;
//!
//! let css = r#"
//!     Button { color: #333; font-size: 14px; }
//!     .primary { background: #0066cc; color: #fff; }
//!     Button:hover { background: #0052a3; }
//! "#;
//! let sheet = CssParser::parse(css).unwrap();
//! assert!(!sheet.rules().is_empty());
//! ```

use core::cell::{Cell, RefCell};

use crate::compat::{format, vec, Box, HashMap, MiniToString, Rc, String, Vec};
use crate::core::Color;
use crate::style::selector::{ElementRef, RuleIndexKey};
use crate::style::{
    BorderStyle, CornerRadii, PseudoState, Selector, StyleRule, StyleSheet, WidgetStyle,
};
use crate::widget::WidgetKind;

/// A declaration value already converted from its source text.
///
/// Stored only in a thread-local cache keyed by the declaration's exact text (G-21): applying the
/// same sheet to every control used to re-run the lexer and the colour/length/gradient parsers for
/// every declaration of every match.
#[derive(Debug, Clone)]
enum CachedDeclValue {
    Color(Color),
    Length(u32),
    Lengths(Vec<u32>),
    Float(f32),
    Gradient(crate::style::Gradient),
    Shadow(crate::style::Shadow),
}

// The most recently parsed declaration values, keyed by kind and a hash of the source text.
// The stored text is compared on a hit, so a hash collision is a miss rather than a wrong value.
struct DeclCacheEntry {
    /// The source text, so a hash collision misses instead of returning a different value.
    text: String,
    /// The converted value.
    value: CachedDeclValue,
    /// Monotonic use stamp; the smallest one is evicted at capacity (G-161).
    last_used: u64,
}

/// A parsed inline declaration list, plus the use stamp that orders eviction.
struct InlineDeclEntry {
    /// The source text, compared on a hash hit.
    text: String,
    /// The split declarations; shared with the caller so a hit clones a pointer, not the list.
    declarations: Rc<Vec<(String, String)>>,
    /// Monotonic use stamp; the smallest one is evicted at capacity (G-161).
    last_used: u64,
}

thread_local! {
    // `HashMap::new` is const only under alloc_frugal (BTreeMap); the std profile cannot be const.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static DECL_CACHE: RefCell<HashMap<(u8, u64), DeclCacheEntry>> =
        RefCell::new(HashMap::new());
    /// Parsed semicolon-separated declaration lists (the inline-CSS form), keyed by text hash.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static INLINE_DECL_CACHE: RefCell<HashMap<u64, InlineDeclEntry>> =
        RefCell::new(HashMap::new());
    /// The monotonically increasing stamp both caches use to order eviction.
    static CACHE_TICK: Cell<u64> = const { Cell::new(0) };
}

/// How many converted declaration values one thread keeps.
const DECL_CACHE_LIMIT: usize = 512;
/// How many inline declaration lists one thread keeps.
const INLINE_DECL_CACHE_LIMIT: usize = 64;

/// The next use stamp, shared by both caches so "oldest" is one global order.
fn next_cache_tick() -> u64 {
    CACHE_TICK.with(|tick| {
        let next = tick.get().wrapping_add(1);
        tick.set(next);
        next
    })
}

/// FNV-1a over declaration text. Cheap (linear in the value) and allocation-free.
fn declaration_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The two CSS combinators the selector grammar supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssCombinator {
    /// Whitespace: any ancestor may match.
    Descendant,
    /// `>`: only the immediate parent may match.
    Child,
}

/// A CSS-parsed selector that stores raw selector parts as strings.
/// Used for matching during CSS rule application.
#[derive(Debug, Clone, PartialEq)]
pub enum CssSelector {
    /// Match any widget.
    Universal,
    /// Match by widget kind name (e.g., "Button", "Label").
    Kind(String),
    /// Match by CSS class name (e.g., ".my-class").
    Class(String),
    /// Match by widget ID (e.g., "#my-id").
    Id(String),
    /// Match a widget that has a specific state (e.g., ":hover").
    State(PseudoState),
    /// AND-combination of multiple selectors.
    And(Vec<CssSelector>),
    /// A descendant combinator (`Ancestor Subject`): the subject matches the element and the
    /// ancestor selector matches at least one ancestor.
    Descendant(Box<CssSelector>, Box<CssSelector>),
    /// A child combinator (`Parent > Subject`): the subject matches the element and the parent
    /// selector matches its immediate parent.
    Child(Box<CssSelector>, Box<CssSelector>),
}

impl CssSelector {
    /// CSS specificity weight, used to order matching rules inside one apply pass.
    ///
    /// The scale mirrors [`Selector::specificity`](crate::style::selector::Selector::specificity)
    /// — id > class > kind — so a rule written for `.primary` wins over the generic `Button` rule
    /// regardless of which one appears first in the sheet. Before this existed, the runtime applied
    /// rules in source order only, which is the opposite of the CSS cascade.
    pub fn specificity(&self) -> u32 {
        match self {
            CssSelector::Universal => 0,
            CssSelector::Kind(_) => 1,
            CssSelector::State(_) => 1,
            CssSelector::Class(_) => 10,
            CssSelector::Id(_) => 100,
            CssSelector::And(selectors) => selectors.iter().map(|s| s.specificity()).sum(),
            // The combinator itself contributes no weight: specificity is the two compounds' sum.
            CssSelector::Descendant(ancestor, subject) | CssSelector::Child(ancestor, subject) => {
                ancestor.specificity() + subject.specificity()
            }
        }
    }

    /// Convert this CSS-parsed selector into the canonical `Selector` enum.
    ///
    /// Returns `None` when the selector names a widget kind the library does not
    /// know. Encoding "unknown kind" as a `Selector` was the previous behaviour and
    /// it was wrong in a way that is hard to notice: an unrecognised name fell back
    /// to `WidgetKind::Window`, so a rule written `Stepper { … }` silently selected
    /// every **window** in the tree. `None` makes the caller decide, and the
    /// alternative — matching nothing — is the honest reading of a rule whose
    /// subject does not exist.
    pub fn to_selector(&self) -> Option<Selector> {
        Some(match self {
            CssSelector::Universal => Selector::Universal,
            CssSelector::Kind(name) => Selector::Kind(widget_kind_from_str(name)?),
            CssSelector::Class(name) => Selector::Class(name.clone()),
            CssSelector::Id(id) => Selector::Id(id.clone()),
            CssSelector::State(state) => Selector::State(*state),
            CssSelector::And(selectors) => Selector::And(
                selectors.iter().map(CssSelector::to_selector).collect::<Option<Vec<_>>>()?,
            ),
            CssSelector::Descendant(ancestor, subject) => Selector::Descendant {
                ancestor: Box::new(ancestor.to_selector()?),
                subject: Box::new(subject.to_selector()?),
            },
            CssSelector::Child(parent, subject) => Selector::Child {
                parent: Box::new(parent.to_selector()?),
                subject: Box::new(subject.to_selector()?),
            },
        })
    }

    /// The narrowing key this selector is filed under in a [`RuleIndex`] (G-162).
    ///
    /// Mirrors the typed selector's `selector_index_key`: the **subject** of a combinator is the
    /// only fact about the element, and one constituent of an `And` is enough to file the rule
    /// under (an element matching the compound satisfies every constituent).
    pub(crate) fn index_key(&self) -> RuleIndexKey<'_> {
        match self {
            CssSelector::Kind(name) => RuleIndexKey::Kind(name),
            CssSelector::Class(name) => RuleIndexKey::Class(name),
            CssSelector::Id(id) => RuleIndexKey::Id(id),
            CssSelector::Universal | CssSelector::State(_) => RuleIndexKey::Any,
            CssSelector::And(selectors) => {
                if let Some(id) = selectors.iter().find_map(|selector| match selector {
                    CssSelector::Id(id) => Some(id.as_str()),
                    _ => None,
                }) {
                    return RuleIndexKey::Id(id);
                }
                if let Some(class) = selectors.iter().find_map(|selector| match selector {
                    CssSelector::Class(class) => Some(class.as_str()),
                    _ => None,
                }) {
                    return RuleIndexKey::Class(class);
                }
                if let Some(kind) = selectors.iter().find_map(|selector| match selector {
                    CssSelector::Kind(kind) => Some(kind.as_str()),
                    _ => None,
                }) {
                    return RuleIndexKey::Kind(kind);
                }
                RuleIndexKey::Any
            }
            CssSelector::Descendant(_, subject) | CssSelector::Child(_, subject) => {
                subject.index_key()
            }
        }
    }

    /// Check if this selector matches a widget with the given properties.
    ///
    /// # How a kind selector is compared
    ///
    /// Not with `eq_ignore_ascii_case`. The `kind` handed in is the widget's
    /// `Debug` spelling (`"MenuBar"`, `"ToolBar"`, `"ProgressBar"`), while a
    /// stylesheet author writes the spelling the rest of the library publishes —
    /// the factory name (`"menu_bar"`, `"tool_bar"`), which is what
    /// `factory_name_for_kind`, the capability registry and the JSON loader all
    /// use. Case folding alone does not bridge the two: `menu_bar` would compare
    /// unequal to `MenuBar` and the rule would apply to nothing, silently, because
    /// the rule still parses and no diagnostic is emitted for a *match* failure.
    ///
    /// `normalize_key` is the crate's single name normaliser (it also strips `_`,
    /// `-` and spaces), so comparing through it makes a selector accept exactly the
    /// spellings the factory accepts.
    pub fn matches(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> bool {
        self.matches_in(kind, class, id, state, &[])
    }

    /// Matches against an element and its ancestor chain.
    ///
    /// `ancestors` is ordered root-first. A combinator rule evaluated without a chain answers
    /// `false`, which is the only honest answer: the condition it describes is a fact about the
    /// tree, not about the element alone.
    pub fn matches_in(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        ancestors: &[ElementRef<'_>],
    ) -> bool {
        match self {
            CssSelector::Universal => true,
            CssSelector::Kind(k) => crate::widget::capability::coercion::same_key(k, kind),
            CssSelector::Class(c) => class == Some(c.as_str()),
            CssSelector::Id(i) => id == Some(i.as_str()),
            CssSelector::State(s) => state == Some(*s),
            CssSelector::And(selectors) => {
                selectors.iter().all(|s| s.matches_in(kind, class, id, state, ancestors))
            }
            CssSelector::Descendant(ancestor, subject) => {
                subject.matches_in(kind, class, id, state, ancestors)
                    && ancestors.iter().any(|candidate| {
                        ancestor.matches_in(
                            candidate.kind.css_name(),
                            candidate.class,
                            candidate.id,
                            candidate.state,
                            &[],
                        )
                    })
            }
            CssSelector::Child(parent, subject) => {
                subject.matches_in(kind, class, id, state, ancestors)
                    && ancestors.last().is_some_and(|candidate| {
                        parent.matches_in(
                            candidate.kind.css_name(),
                            candidate.class,
                            candidate.id,
                            candidate.state,
                            &[],
                        )
                    })
            }
        }
    }
}

/// A parsed CSS declaration: property name → raw value string.
///
/// The value is **not** interpreted here — it stays as written in the
/// stylesheet, so the consumer of a declaration is responsible for parsing
/// units and rejecting values it cannot apply.
#[derive(Debug, Clone, PartialEq)]
pub struct CssDeclaration {
    /// Property name exactly as written in the source, for example
    /// `"background-color"`. Not normalised, so case variants are distinct keys.
    pub property: String,
    /// Raw value text, for example `"#ff0000"` or `"12px"`. Unparsed and
    /// untrimmed beyond what the tokeniser already removed.
    pub value: String,
}

/// A parsed CSS rule: selector text → declarations.
///
/// The selector is kept as text rather than a parsed selector type, so the rule
/// remembers how it was written; matching happens elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct CssRule {
    /// The selector as it appeared in the source, for example
    /// `"Button.primary:hover"`.
    pub selector_text: String,
    /// The rule's declarations, in source order. No deduplication is performed,
    /// so a property written twice appears twice.
    pub declarations: Vec<CssDeclaration>,
}

/// A rule whose selector has been parsed **once**, plus its cascade weight (G-164).
///
/// # Why this type exists
///
/// The apply path and the trace both need "these rules, with their selectors resolved".
/// [`StyleSheetManager`](crate::style::stylesheet::StyleSheetManager) parses at registration and
/// the trace used to re-parse every sheet on every call — including
/// [`CssParser::parse_rules`]'s whole-text `chars()` expansion. Handing both the same prepared
/// rule set means a trace costs a selector match and nothing else, and the two consumers cannot
/// disagree about how a selector parsed. `selector` is `None` when the text is not a selector this
/// parser accepts; a caller reports that rather than silently dropping the rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCssRule {
    /// The selector as it appeared in the source.
    pub selector_text: String,
    /// The parsed selector, or `None` when the text is not a selector this parser accepts.
    pub selector: Option<CssSelector>,
    /// Cascade weight of `selector`, computed once here rather than per apply (G-162).
    pub specificity: u32,
    /// The rule's declarations, in source order.
    pub declarations: Vec<CssDeclaration>,
}

/// Convert a widget kind name string (e.g. "Button", "Label") to a `WidgetKind` variant.
///
/// Returns `None` for a name that is not a known kind. The previous version
/// returned `WidgetKind::Window` as a "safe default", which was not safe: a rule
/// whose subject the parser did not recognise would match windows instead of
/// matching nothing, and the resulting mis-styling gave no hint about the cause.
///
/// Resolves a CSS kind selector's spelling to a [`WidgetKind`].
///
/// # Why this is not a hand-written `match`
///
/// It used to be one, and it drifted: the table held three multi-word kinds
/// (`ScrollArea`, `ScrollBar`, `GroupBox` and friends) but not `MenuBar`, `ToolBar`,
/// `StatusBar`, `ProgressBar`, `TabWidget` or `MenuButton`, and every arm compared
/// with `eq_ignore_ascii_case`. The consequence was silent — `menu_bar { … }` on the
/// `to_selector` path resolved to `None`, so
/// `StyleSheetManager::apply_to` matched nothing and the stylesheet was ignored with
/// no diagnostic, while the same text spelled `MenuBar` worked.
///
/// The list below is therefore a **coverage list**, not a name table: each entry is a
/// kind the CSS layer intends to support, and the comparison itself goes through
/// [`normalize_key`], the crate's single name normaliser — the same one the widget
/// factory uses to register canonical names and aliases. A selector therefore accepts
/// exactly the spellings the factory accepts (`menu_bar`, `MenuBar`, `menubar`, …),
/// and the two cannot drift apart again.
///
/// A kind absent from the list is refused rather than guessed at, so an unknown
/// selector still selects nothing instead of silently becoming `Window`.
fn widget_kind_from_str(name: &str) -> Option<WidgetKind> {
    use crate::widget::capability::coercion::same_key;
    // `css_name` rather than `format!("{kind:?}")`: the comparison used to format every
    // candidate kind on every selector resolution, which is a heap allocation and a formatting
    // pass per entry per lookup. The spelling is identical by construction
    // (`css_name_matches_the_debug_spelling_for_every_registered_kind` pins it).
    CSS_SUPPORTED_KINDS.iter().copied().find(|kind| same_key(kind.css_name(), name))
}

/// The kinds a CSS kind selector may name.
///
/// Ordered for readability, grouped by area. Every entry must be a kind the widget
/// factory can also create under some spelling; `every_kind_selector_spelling_resolves`
/// checks each one resolves through both paths.
///
/// # Why entries are gated
///
/// `kind.rs` gates many variants on `widgets_unstripped`, so naming one ungated makes
/// `mini`/`embedded` fail to compile with `no variant named X`. The gates here must
/// therefore mirror `kind.rs` exactly. An earlier revision of this list (a
/// hand-written `match`) carried only six such cfg attributes while listing
/// twenty-eight gated variants, which is why `mini` was 47 errors — the list is now
/// split so the requirement is visible rather than remembered.
const CSS_SUPPORTED_KINDS: &[WidgetKind] = &[
    // ── Always available (no gate in `kind.rs`) ──
    WidgetKind::Window,
    WidgetKind::Frame,
    WidgetKind::Label,
    WidgetKind::Button,
    WidgetKind::CheckBox,
    WidgetKind::RadioButton,
    WidgetKind::GroupBox,
    WidgetKind::Panel,
    WidgetKind::Line,
    WidgetKind::Arc,
    WidgetKind::Meter,
    WidgetKind::Slider,
    WidgetKind::Spinner,
    WidgetKind::Roller,
    WidgetKind::Switch,
    WidgetKind::LineEdit,
    WidgetKind::ComboBox,
    WidgetKind::SpinBox,
    WidgetKind::ListBox,
    WidgetKind::ScrollBar,
    WidgetKind::ScrollArea,
    WidgetKind::ProgressBar,
    WidgetKind::ImageView,
    WidgetKind::MiniChart,
    WidgetKind::MiniCanvas,
    WidgetKind::Keyboard,
    WidgetKind::Dropdown,
    // ── `widgets_unstripped` (same gate as their declaration in `kind.rs`) ──
    #[cfg(widgets_unstripped)]
    WidgetKind::ToggleButton,
    #[cfg(widgets_unstripped)]
    WidgetKind::TabWidget,
    #[cfg(widgets_unstripped)]
    WidgetKind::TabBar,
    #[cfg(widgets_unstripped)]
    WidgetKind::StackedWidget,
    #[cfg(widgets_unstripped)]
    WidgetKind::Splitter,
    #[cfg(widgets_unstripped)]
    WidgetKind::StatusBar,
    #[cfg(widgets_unstripped)]
    WidgetKind::MenuBar,
    #[cfg(widgets_unstripped)]
    WidgetKind::Menu,
    #[cfg(widgets_unstripped)]
    WidgetKind::MenuButton,
    #[cfg(widgets_unstripped)]
    WidgetKind::ToolBar,
    #[cfg(widgets_unstripped)]
    WidgetKind::ToolButton,
    #[cfg(widgets_unstripped)]
    WidgetKind::Badge,
    #[cfg(widgets_unstripped)]
    WidgetKind::Rating,
    #[cfg(widgets_unstripped)]
    WidgetKind::Divider,
    #[cfg(widgets_unstripped)]
    WidgetKind::ProgressCircle,
    #[cfg(widgets_unstripped)]
    WidgetKind::LCDNumber,
    #[cfg(widgets_unstripped)]
    WidgetKind::SkeletonLoader,
    // ── `full_widgets` (a device profile **and** unstripped) ──
    #[cfg(full_widgets)]
    WidgetKind::RadarChart,
    #[cfg(full_widgets)]
    WidgetKind::Heatmap,
    #[cfg(full_widgets)]
    WidgetKind::KanbanBoard,
    #[cfg(full_widgets)]
    WidgetKind::Cascader,
    #[cfg(full_widgets)]
    WidgetKind::QueryBuilder,
    #[cfg(full_widgets)]
    WidgetKind::EmojiPicker,
    #[cfg(full_widgets)]
    WidgetKind::Mention,
];

/// The property names [`CssParser::apply_value`] acts on.
///
/// This is the parser's vocabulary, not a second implementation: every entry has a match arm in
/// `apply_value`, and the behavioural test `known_property_list_matches_what_apply_one_handles`
/// iterates this array to prove each one actually changes a style (G-404).
const KNOWN_PROPERTIES: &[&str] = &[
    "color",
    "text-color",
    "foreground",
    "background",
    "background-color",
    "border-color",
    "border-width",
    "border-style",
    "border-radius",
    "font-size",
    "font-weight",
    "font-family",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "opacity",
    "background-gradient",
    "shadow",
    "touch-target",
    "touch-target-size",
];

/// Where [`CssParser::apply_value_with`] sends the value it parsed.
///
/// Two destinations exist so the trace can ask "would this declaration apply?" by running the
/// parser's **own** match arms without building a scratch [`WidgetStyle`] (G-164). `Apply` writes
/// into a style; `Check` discards the value after it has been parsed, which is exactly the
/// judgement the trace needs. One table, two consumers — a second validation table is what would
/// let the two disagree about whether a declaration is valid.
enum ValueSink<'a> {
    /// Write into the caller's style.
    Apply(&'a mut WidgetStyle),
    /// Parse only: the value is validated and thrown away.
    Check,
}

impl ValueSink<'_> {
    fn text_color(&mut self, color: Color) {
        if let Self::Apply(style) = self {
            style.text_color = Some(color);
        }
    }

    fn background_color(&mut self, color: Color) {
        if let Self::Apply(style) = self {
            style.background_color = Some(color);
        }
    }

    fn border_color(&mut self, color: Color) {
        if let Self::Apply(style) = self {
            style.border_color = Some(color);
        }
    }

    fn border_width(&mut self, width: u32) {
        if let Self::Apply(style) = self {
            style.border_width = Some(width);
        }
    }

    fn border_style(&mut self, border_style: BorderStyle) {
        if let Self::Apply(style) = self {
            style.border_style = Some(border_style);
        }
    }

    /// Writes the four corners, and keeps the historical scalar in step when it can express the
    /// same shape (STY-01).
    fn corner_radii(&mut self, radii: CornerRadii) {
        if let Self::Apply(style) = self {
            style.corner_radii = Some(radii);
            if radii.is_uniform() {
                style.border_radius = Some(radii.top_left);
            }
        }
    }

    fn padding(&mut self, padding: crate::style::Padding) {
        if let Self::Apply(style) = self {
            style.padding = padding;
        }
    }

    fn padding_top(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.padding.top = value;
        }
    }

    fn padding_right(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.padding.right = value;
        }
    }

    fn padding_bottom(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.padding.bottom = value;
        }
    }

    fn padding_left(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.padding.left = value;
        }
    }

    fn margin(&mut self, margin: crate::style::Margin) {
        if let Self::Apply(style) = self {
            style.margin = margin;
        }
    }

    fn margin_top(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.margin.top = value;
        }
    }

    fn margin_right(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.margin.right = value;
        }
    }

    fn margin_bottom(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.margin.bottom = value;
        }
    }

    fn margin_left(&mut self, value: u32) {
        if let Self::Apply(style) = self {
            style.margin.left = value;
        }
    }

    fn opacity(&mut self, opacity: f32) {
        if let Self::Apply(style) = self {
            style.opacity = Some(opacity);
        }
    }

    fn gradient(&mut self, gradient: crate::style::Gradient) {
        if let Self::Apply(style) = self {
            style.background_gradient = Some(gradient);
        }
    }

    fn shadow(&mut self, shadow: Option<crate::style::Shadow>) {
        if let Self::Apply(style) = self {
            style.shadow = shadow;
        }
    }

    fn touch_target(&mut self, size: crate::core::Size) {
        if let Self::Apply(style) = self {
            style.touch_target = Some(size);
        }
    }

    /// Reads-modifies-writes the style's font, creating the default one when the style has none.
    ///
    /// In `Check` mode the closure is never built, so validation allocates no font.
    fn font(&mut self, update: impl FnOnce(&mut crate::core::Font)) {
        if let Self::Apply(style) = self {
            match &mut style.font {
                Some(font) => update(font),
                None => {
                    let mut font = crate::core::Font::default_ui();
                    update(&mut font);
                    style.font = Some(font);
                }
            }
        }
    }
}

/// CSS parser that converts CSS text into `StyleSheet` + property application.
pub struct CssParser;

impl CssParser {
    /// Validates CSS text and returns a sheet of the rules whose selectors name a known kind.
    ///
    /// The sheet carries selectors and specificity, **not** their declarations: declarations are
    /// applied by [`Self::parse_and_apply`], and [`Self::parse_rules`] is the inspector's view of
    /// them. A caller that only needs the syntax check — a stylesheet hot-reload watcher, which is
    /// what [`crate::style::css_watcher`] does — can discard the result.
    ///
    /// This used to also copy every rule's declarations into a process-wide side table, for a
    /// `get_declarations` reader that nothing outside the tests ever called. That copy took a
    /// global lock and cloned each declaration slice, on a path a watcher runs on every file
    /// change and a JSON document can run once per node. The table and its reader are gone; the
    /// declarations still reach a control through `parse_and_apply`.
    pub fn parse(css: &str) -> Result<StyleSheet, String> {
        let mut sheet = StyleSheet::new();
        let rules = Self::parse_rules(css)?;
        for rule in &rules {
            // A selector whose kind is unknown contributes no rule: silently
            // substituting another kind is what made a typo'd selector mis-style
            // an unrelated control.
            let Some(selector) =
                Self::parse_selector(&rule.selector_text).and_then(|cs| cs.to_selector())
            else {
                log::warn!(
                    "CSS rule selector {:?} names no known widget kind and was not registered as a \
                     rule",
                    rule.selector_text
                );
                continue;
            };
            let rule_entry = StyleRule::new(selector, &rule.selector_text);
            sheet.add_rule(rule_entry);
        }
        Ok(sheet)
    }

    /// Parse CSS text into raw `CssRule` entries.
    ///
    /// Public because a CSS **inspector** must walk the same rules the apply does: exposing the
    /// parsed rules is what lets `style::css_trace` report a winning rule without a second parser
    /// (BLUE26 B26.10).
    pub fn parse_rules(css: &str) -> Result<Vec<CssRule>, String> {
        let mut rules = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = css.chars().collect();

        while pos < chars.len() {
            // Skip whitespace and comments
            pos = Self::skip_whitespace_and_comments(&chars, pos);
            if pos >= chars.len() {
                break;
            }

            // Read selector text until '{'
            let start = pos;
            while pos < chars.len() && chars[pos] != '{' {
                pos += 1;
            }
            if pos >= chars.len() {
                return Err(
                    "CSS rule is unterminated: the `{` that opens the declaration block is \
                     missing at end of input"
                        .to_string(),
                );
            }
            let selector_text: String = chars[start..pos].iter().collect();
            let selector_text = selector_text.trim().to_string();
            if selector_text.is_empty() {
                return Err(
                    "CSS rule has an empty selector before `{`; every rule needs a selector \
                     such as `Button` or `.primary`"
                        .to_string(),
                );
            }

            pos += 1; // skip '{'

            // Read declarations until '}'
            let decl_start = pos;
            while pos < chars.len() && chars[pos] != '}' {
                pos += 1;
            }
            if pos >= chars.len() {
                return Err(
                    "CSS rule is unterminated: the declaration block is missing its closing \
                     `}` at end of input"
                        .to_string(),
                );
            }
            let decl_text: String = chars[decl_start..pos].iter().collect();
            pos += 1; // skip '}'

            let declarations = Self::parse_declarations(&decl_text);
            rules.push(CssRule { selector_text, declarations });
        }

        Ok(rules)
    }

    /// Parses CSS text into rules whose selectors are resolved once (G-164).
    ///
    /// This is the parse a registered sheet keeps and a trace consumes, so neither re-expands the
    /// source text nor re-parses a selector per call. A selector this parser cannot express is
    /// carried as `None` rather than dropped here: the registration path filters it (an unmatched
    /// rule is the honest reading of an unknown kind) while the trace reports it.
    pub fn parse_prepared_rules(css: &str) -> Result<Vec<ParsedCssRule>, String> {
        Ok(Self::parse_rules(css)?
            .into_iter()
            .map(|rule| {
                let selector = Self::parse_selector(&rule.selector_text);
                let specificity = selector.as_ref().map_or(0, CssSelector::specificity);
                ParsedCssRule {
                    selector_text: rule.selector_text,
                    selector,
                    specificity,
                    declarations: rule.declarations,
                }
            })
            .collect())
    }

    /// Parse a selector string into a `CssSelector`.
    ///
    /// Supports compounds (`Button`, `.class`, `#id`, `:hover`, `Button.class`,
    /// `Button:hover`) joined by the CSS combinators: whitespace for a descendant and `>` for a
    /// child, so `Panel Button` and `Panel > Button` are distinct selectors. A chain is built
    /// right-associatively, because the rightmost compound is always the element being styled.
    ///
    /// A chain needs the element's ancestors to evaluate — see
    /// [`CssSelector::matches_in`] — so `matches` alone answers `false` for a combinator rule
    /// instead of treating it as an unconditional match.
    pub fn parse_selector(text: &str) -> Option<CssSelector> {
        let text = text.trim();
        if text.is_empty() || text == "*" {
            return Some(CssSelector::Universal);
        }

        let (parts, combinators) = Self::split_combinators(text);
        if combinators.len() != parts.len().saturating_sub(1) || parts.is_empty() {
            return None;
        }
        let mut parts = parts.into_iter();
        let mut selector = Self::parse_compound(&parts.next()?)?;
        let mut combinators = combinators.into_iter();
        for part in parts {
            let subject = Self::parse_compound(&part)?;
            selector = match combinators.next()? {
                CssCombinator::Descendant => {
                    CssSelector::Descendant(Box::new(selector), Box::new(subject))
                }
                CssCombinator::Child => CssSelector::Child(Box::new(selector), Box::new(subject)),
            };
        }
        Some(selector)
    }

    /// Parses one compound selector (no combinators) into a [`CssSelector`].
    fn parse_compound(text: &str) -> Option<CssSelector> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if text == "*" {
            return Some(CssSelector::Universal);
        }
        let parts = Self::split_selector_parts(text);
        let mut selectors = Vec::new();

        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some(class_name) = part.strip_prefix('.') {
                selectors.push(CssSelector::Class(class_name.to_string()));
            } else if let Some(id_name) = part.strip_prefix('#') {
                selectors.push(CssSelector::Id(id_name.to_string()));
            } else if let Some(state_name) = part.strip_prefix(':') {
                // An unrecognised pseudo-class used to be dropped silently, which left the rest of
                // the selector in place: `Button:hoover` (typo) therefore parsed to `Button` and
                // painted every button as if it were hovered, forever. A selector whose condition
                // cannot be expressed must match nothing instead (the same policy
                // `to_selector` already applies to an unknown kind).
                let state = Self::parse_pseudo_state(state_name)?;
                selectors.push(CssSelector::State(state));
            } else if part == "*" {
                selectors.push(CssSelector::Universal);
            } else {
                // Try parsing as widget kind
                selectors.push(CssSelector::Kind(part.to_string()));
            }
        }

        match selectors.len() {
            0 => None,
            1 => Some(selectors.into_iter().next().unwrap()),
            _ => Some(CssSelector::And(selectors)),
        }
    }

    /// Splits a complex selector on its top-level combinators.
    ///
    /// Returns the compound texts and the joins between them (`parts.len() - 1` joins). Runs of
    /// whitespace collapse to one descendant join, and whitespace around `>` does not create an
    /// extra join, so `A  >  B` and `A>B` describe the same selector.
    fn split_combinators(text: &str) -> (Vec<String>, Vec<CssCombinator>) {
        let mut parts = Vec::new();
        let mut joins = Vec::new();
        let mut current = String::new();
        let mut pending_descendant = false;

        let flush = |current: &mut String, parts: &mut Vec<String>| {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
            current.clear();
        };

        for ch in text.chars() {
            match ch {
                '>' => {
                    flush(&mut current, &mut parts);
                    pending_descendant = false;
                    // A `>` may end the text (malformed); the missing subject then makes the
                    // selector fail in `parse_selector`, which is the honest answer.
                    joins.push(CssCombinator::Child);
                }
                c if c.is_whitespace() => {
                    if !current.trim().is_empty() {
                        flush(&mut current, &mut parts);
                        pending_descendant = true;
                    }
                }
                c => {
                    if pending_descendant && !parts.is_empty() {
                        joins.push(CssCombinator::Descendant);
                        pending_descendant = false;
                    }
                    current.push(c);
                }
            }
        }
        flush(&mut current, &mut parts);

        // A trailing join has no compound after it; `parse_selector` rejects the length mismatch
        // rather than pairing the join with nothing.
        (parts, joins)
    }

    /// Split a compound selector into parts (e.g., "Button.primary:hover" → ["Button", ".primary", ":hover"])
    fn split_selector_parts(text: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            if ch == '.' || ch == '#' || ch == ':' {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
                current.push(ch);
            } else if ch.is_whitespace() {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            parts.push(current);
        }
        parts
    }

    fn parse_pseudo_state(name: &str) -> Option<PseudoState> {
        // One spelling source drives both directions: the canonical `WidgetState` suffix table
        // plus its documented aliases (G-405). A `match` here used to hold a second copy of the
        // vocabulary, so a state added to the enum had to be remembered in this function too.
        PseudoState::from_state_suffix(name)
    }

    /// Parse declaration block text into a list of declarations.
    fn parse_declarations(text: &str) -> Vec<CssDeclaration> {
        let mut decls = Vec::new();
        for line in text.split(';') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(idx) = line.find(':') {
                let property = line[..idx].trim().to_string();
                let value = line[idx + 1..].trim().to_string();
                if !property.is_empty() && !value.is_empty() {
                    decls.push(CssDeclaration { property, value });
                }
            }
        }
        decls
    }

    /// Apply parsed declarations from matched rules to a `WidgetStyle`.
    pub fn apply_declarations(
        declarations: &[CssDeclaration],
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        for decl in declarations {
            Self::apply_one(decl, style)?;
        }
        // An authored `shadow:` must reach the face the renderer paints; see
        // `WidgetStyle::reconcile_shadow_into_surface` for why the two spellings exist and what
        // went wrong when nothing connected them. Done once per block rather than per declaration
        // because that is what the surface represents: the *result* of every declaration applied
        // so far.
        style.reconcile_shadow_into_surface();
        Ok(())
    }

    /// Applies one declaration written as `"property: value"`.
    ///
    /// # Why this takes text rather than a parsed declaration
    ///
    /// Callers that hold a ``"background-color: #FFF"`` string — the C ABI and the
    /// JSON path both do — should not have to construct a [`CssDeclaration`], which
    /// would mean either exposing the struct or duplicating the `":"` split. Taking
    /// the text keeps one parser for both.
    ///
    /// # Why an unparseable declaration is an error
    ///
    /// A string with no `":"`, or with an empty side, is rejected rather than
    /// ignored. The stylesheet path deliberately skips such lines, because a
    /// hand-written stylesheet may carry comments and half-finished edits; a
    /// programmatic single-property write has no such excuse, and silently doing
    /// nothing would leave the caller believing the style applied.
    ///
    /// # Why this cannot report an unknown property
    ///
    /// `Self::apply_one` ignores unknown properties, which is what the CSS spec
    /// requires of a *stylesheet* — a sheet written for one renderer must load in
    /// another that supports fewer properties. That tolerance is right for a sheet and
    /// wrong for a single programmatic write, where the caller typed one property and
    /// nothing else: a misspelling there is a bug, and reporting it as success hides
    /// it. The ABI therefore checks the property name against this parser's vocabulary
    /// before applying, using [`Self::is_known_property`].
    pub fn apply_declaration_text(text: &str, style: &mut WidgetStyle) -> Result<(), String> {
        // A declaration list is the form a widget's own CSS text takes (`"a: 1; b: 2"`), so the
        // list form is accepted here rather than leaving each caller to split it — two splitters
        // is how "one trailing semicolon is fine in a sheet but not inline" would happen.
        if text.contains(';') {
            return Self::apply_declarations_text(text, style);
        }
        let (property, value) = Self::split_declaration(text)?;
        Self::apply_value(property, value, style)?;
        // The single-declaration path reconciles too, so a programmatic write behaves exactly
        // like the same line inside a stylesheet.
        style.reconcile_shadow_into_surface();
        Ok(())
    }

    /// Applies a semicolon-separated declaration list.
    ///
    /// Empty segments (a trailing `;`, a blank line) are skipped rather than reported: a
    /// declaration list written as CSS ends with one by convention.
    pub fn apply_declarations_text(text: &str, style: &mut WidgetStyle) -> Result<(), String> {
        let Ok(declarations) = Self::cached_inline_declarations(text) else {
            // Preserve the old prefix-then-error behaviour: an invalid segment reports the same
            // error after the valid declarations before it have been applied.
            for segment in text.split(';') {
                let segment = segment.trim();
                if segment.is_empty() {
                    continue;
                }
                Self::apply_declaration_text(segment, style)?;
            }
            return Ok(());
        };
        for (property, value) in declarations.iter() {
            Self::apply_value(property, value, style)?;
            style.reconcile_shadow_into_surface();
        }
        Ok(())
    }

    /// Splits and trims an inline declaration list once per distinct text (G-21).
    fn cached_inline_declarations(text: &str) -> Result<Rc<Vec<(String, String)>>, String> {
        let hash = declaration_hash(text);
        let hit = INLINE_DECL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let entry = cache.get_mut(&hash)?;
            if entry.text != text {
                return None;
            }
            // Refresh the stamp so eviction takes the genuinely coldest entry (G-161).
            entry.last_used = next_cache_tick();
            Some(Rc::clone(&entry.declarations))
        });
        if let Some(hit) = hit {
            return Ok(hit);
        }
        let mut declarations = Vec::new();
        for segment in text.split(';') {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            let (property, value) = Self::split_declaration(segment)?;
            declarations.push((String::from(property), String::from(value)));
        }
        let declarations = Rc::new(declarations);
        INLINE_DECL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() >= INLINE_DECL_CACHE_LIMIT {
                // Same single-entry eviction as the declaration cache (G-161).
                let victim =
                    cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(key, _)| *key);
                if let Some(victim) = victim {
                    cache.remove(&victim);
                }
            }
            cache.insert(
                hash,
                InlineDeclEntry {
                    text: String::from(text),
                    declarations: Rc::clone(&declarations),
                    last_used: next_cache_tick(),
                },
            );
        });
        Ok(declarations)
    }

    /// Splits `"property: value"`, rejecting the shapes that cannot be a declaration.
    fn split_declaration(text: &str) -> Result<(&str, &str), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("empty style declaration".to_string());
        }
        let Some(index) = trimmed.find(':') else {
            return Err(format!(
                "style declaration {trimmed:?} has no ':' separator; expected \"property: value\""
            ));
        };
        let property = trimmed[..index].trim();
        let value = trimmed[index + 1..].trim();
        if property.is_empty() {
            return Err(format!("style declaration {trimmed:?} has an empty property name"));
        }
        if value.is_empty() {
            return Err(format!(
                "style declaration {trimmed:?} has an empty value for property {property:?}"
            ));
        }
        Ok((property, value))
    }

    /// Whether `property` is one this parser acts on.
    ///
    /// # Why this is a list and not a second match
    ///
    /// The alternative was to make [`Self::apply_value`] return whether it matched, but that
    /// would tempt a caller to treat the unknown case as an error everywhere — including in the
    /// stylesheet path, where ignoring is correct. Naming the vocabulary separately keeps the two
    /// policies independent.
    ///
    /// # Why the list is now the same set the apply handles (G-404)
    ///
    /// The list used to name `border-style` and `font-weight` with **no** arm behind them, so a
    /// declaration was accepted, cached and silently dropped, while `css_trace` used this method
    /// to decide "known" and therefore never reported the two as inert. The list is now
    /// [`KNOWN_PROPERTIES`], `apply_value` implements every entry, and
    /// `known_property_list_matches_what_apply_one_handles` iterates the list itself (rather than
    /// a hand-copied subset) so the two cannot drift apart again.
    ///
    /// Matching is ASCII case-insensitive, which is what CSS specifies for property names;
    /// [`Self::apply_value`] applies the same rule, so "known" and "applies" agree for every
    /// spelling.
    pub fn is_known_property(property: &str) -> bool {
        KNOWN_PROPERTIES.iter().any(|known| known.eq_ignore_ascii_case(property))
    }

    /// The property names this parser acts on, in the order they are documented.
    ///
    /// One list, read by [`Self::is_known_property`] and by the trace (through it), and kept in
    /// step with [`Self::apply_value`]'s arms by a behavioural test (G-404).
    pub fn known_properties() -> &'static [&'static str] {
        KNOWN_PROPERTIES
    }

    /// Returns one already-converted declaration value, parsing it at most once per exact text.
    ///
    /// The key is `(kind, hash)` and the stored source text is compared on a hit, so a collision
    /// misses instead of returning a value for different text (G-21).
    fn cached_decl<T: Clone>(
        kind: u8,
        value: &str,
        parse: impl Fn(&str) -> Result<CachedDeclValue, String>,
        extract: impl Fn(&CachedDeclValue) -> Option<T>,
    ) -> Result<T, String> {
        let key = (kind, declaration_hash(value));
        let hit = DECL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let entry = cache.get_mut(&key)?;
            if entry.text != value {
                // Another text hashed to the same key: treat it as a miss rather than returning a
                // value for different text.
                return None;
            }
            // Refresh the stamp so eviction takes the genuinely coldest entry (G-161).
            entry.last_used = next_cache_tick();
            Some(entry.value.clone())
        });
        if let Some(parsed) = hit {
            if let Some(extracted) = extract(&parsed) {
                return Ok(extracted);
            }
        }
        let parsed = parse(value)?;
        let extracted = extract(&parsed).ok_or_else(|| {
            format!("internal: declaration {value:?} did not produce its expected value kind")
        })?;
        DECL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() >= DECL_CACHE_LIMIT {
                // Evict the single coldest entry rather than clearing the table (G-161). A clear
                // threw away every warm value at once on the 513th distinct declaration, and the
                // next apply re-parsed all of them.
                let victim =
                    cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(key, _)| *key);
                if let Some(victim) = victim {
                    cache.remove(&victim);
                }
            }
            cache.insert(
                key,
                DeclCacheEntry {
                    text: String::from(value),
                    value: parsed,
                    last_used: next_cache_tick(),
                },
            );
        });
        Ok(extracted)
    }

    fn cached_color(value: &str) -> Result<Color, String> {
        Self::cached_decl(
            0,
            value,
            |value| Self::parse_color(value).map(CachedDeclValue::Color),
            |parsed| match parsed {
                CachedDeclValue::Color(color) => Some(*color),
                _ => None,
            },
        )
    }

    fn cached_length(value: &str) -> Result<u32, String> {
        Self::cached_decl(
            1,
            value,
            |value| Self::parse_length(value).map(CachedDeclValue::Length),
            |parsed| match parsed {
                CachedDeclValue::Length(length) => Some(*length),
                _ => None,
            },
        )
    }

    fn cached_lengths(value: &str, count: usize) -> Result<Vec<u32>, String> {
        Self::cached_decl(
            2 + count as u8,
            value,
            |value| Self::parse_space_separated_lengths(value, count).map(CachedDeclValue::Lengths),
            |parsed| match parsed {
                CachedDeclValue::Lengths(lengths) => Some(lengths.clone()),
                _ => None,
            },
        )
    }

    fn cached_float(value: &str) -> Result<f32, String> {
        Self::cached_decl(
            8,
            value.trim(),
            |value| Self::parse_float(value).map(CachedDeclValue::Float),
            |parsed| match parsed {
                CachedDeclValue::Float(number) => Some(*number),
                _ => None,
            },
        )
    }

    fn cached_gradient(value: &str) -> Result<crate::style::Gradient, String> {
        Self::cached_decl(
            9,
            value,
            |value| Self::parse_gradient(value).map(CachedDeclValue::Gradient),
            |parsed| match parsed {
                CachedDeclValue::Gradient(gradient) => Some(gradient.clone()),
                _ => None,
            },
        )
    }

    fn cached_shadow(value: &str) -> Result<crate::style::Shadow, String> {
        Self::cached_decl(
            10,
            value,
            |value| Self::parse_shadow(value).map(CachedDeclValue::Shadow),
            |parsed| match parsed {
                CachedDeclValue::Shadow(shadow) => Some(shadow.clone()),
                _ => None,
            },
        )
    }

    /// Empties this thread's declaration cache.
    ///
    /// Test-only: the cache is thread-local internal state, and a test that measures its contents
    /// must start from a known point rather than from whatever an earlier test on the same worker
    /// thread left behind (G-161).
    #[cfg(test)]
    fn clear_decl_cache() {
        DECL_CACHE.with(|cache| cache.borrow_mut().clear());
    }

    /// How many converted declaration values this thread holds. Test-only (G-161).
    #[cfg(test)]
    fn decl_cache_len() -> usize {
        DECL_CACHE.with(|cache| cache.borrow().len())
    }

    /// Whether `value` is currently cached as a length. Test-only (G-161).
    #[cfg(test)]
    fn decl_cache_contains(value: &str) -> bool {
        let key = (1u8, declaration_hash(value));
        DECL_CACHE.with(|cache| cache.borrow().get(&key).is_some_and(|entry| entry.text == value))
    }

    /// Apply a single CSS declaration to a WidgetStyle.
    fn apply_one(decl: &CssDeclaration, style: &mut WidgetStyle) -> Result<(), String> {
        Self::apply_value(&decl.property, &decl.value, style)
    }

    /// Checks one declaration's property and value without writing a style.
    ///
    /// This is the trace's "would this apply?" question (G-164): it runs the **same** match arms
    /// as [`Self::apply_value`] through the `Check` sink, so the verdict is the apply's own rather
    /// than a second rule set's, and no scratch [`WidgetStyle`] is built or mutated. An unknown
    /// property is not an error here — the apply ignores it by design — so a caller that needs
    /// to distinguish the two asks [`Self::is_known_property`] first, as the trace does.
    pub fn validate_declaration(declaration: &CssDeclaration) -> Result<(), String> {
        Self::apply_value_with(&declaration.property, &declaration.value, &mut ValueSink::Check)
    }

    /// Applies one already-split declaration. Borrowing the halves is what lets the inline-list
    /// cache apply on a hit without cloning the property and value strings (G-21).
    fn apply_value(property: &str, value: &str, style: &mut WidgetStyle) -> Result<(), String> {
        Self::apply_value_with(property, value, &mut ValueSink::Apply(style))
    }

    /// The shared declaration writer behind [`Self::apply_value`] and
    /// [`Self::validate_declaration`].
    ///
    /// Property names are ASCII case-insensitive in CSS. The exact spelling is tried first (the
    /// hot path: a parsed sheet already carries the lowercase form), and only a spelling that
    /// actually contains an uppercase letter pays for a lowercase copy. That keeps "known" and
    /// "applies" agreeing for every spelling without allocating on the common path (G-404).
    fn apply_value_with(
        property: &str,
        value: &str,
        sink: &mut ValueSink<'_>,
    ) -> Result<(), String> {
        if Self::apply_value_spelled(property, value, sink)? {
            return Ok(());
        }
        if property.bytes().any(|byte| byte.is_ascii_uppercase()) {
            let lowered = property.to_ascii_lowercase();
            if Self::apply_value_spelled(&lowered, value, sink)? {
                return Ok(());
            }
        }
        // Unknown property — ignore (CSS spec: ignore unknown properties)
        Ok(())
    }

    /// One match arm per entry in [`KNOWN_PROPERTIES`]; `Ok(false)` means "not a property this
    /// parser acts on".
    fn apply_value_spelled(
        property: &str,
        value: &str,
        sink: &mut ValueSink<'_>,
    ) -> Result<bool, String> {
        match property {
            "color" | "text-color" | "foreground" => {
                sink.text_color(Self::cached_color(value)?);
            }
            "background" | "background-color" => {
                sink.background_color(Self::cached_color(value)?);
            }
            "border-color" => {
                sink.border_color(Self::cached_color(value)?);
            }
            "border-width" => {
                sink.border_width(Self::cached_length(value)?);
            }
            "border-style" => {
                sink.border_style(Self::parse_border_style(value)?);
            }
            "border-radius" => {
                // G-404 / STY-01: the CSS 1-4 value shorthand, mapped by the same helper the
                // `padding` and `margin` shorthands use (top-left, top-right, bottom-right,
                // bottom-left). One value still means "all four corners", so an existing sheet
                // written `border-radius: 4px` produces exactly the shape it did before.
                let vals = Self::cached_lengths(value, 4)?;
                sink.corner_radii(CornerRadii::new(vals[0], vals[1], vals[2], vals[3]));
            }
            "padding" => {
                let vals = Self::cached_lengths(value, 4)?;
                sink.padding(crate::style::Padding::new(vals[0], vals[1], vals[2], vals[3]));
            }
            "padding-top" => sink.padding_top(Self::cached_length(value)?),
            "padding-right" => sink.padding_right(Self::cached_length(value)?),
            "padding-bottom" => sink.padding_bottom(Self::cached_length(value)?),
            "padding-left" => sink.padding_left(Self::cached_length(value)?),
            "margin" => {
                let vals = Self::cached_lengths(value, 4)?;
                sink.margin(crate::style::Margin::new(vals[0], vals[1], vals[2], vals[3]));
            }
            "margin-top" => sink.margin_top(Self::cached_length(value)?),
            "margin-right" => sink.margin_right(Self::cached_length(value)?),
            "margin-bottom" => sink.margin_bottom(Self::cached_length(value)?),
            "margin-left" => sink.margin_left(Self::cached_length(value)?),
            "opacity" => {
                let val =
                    Self::parse_float(value).map_err(|_| format!("Invalid opacity: {value}"))?;
                // D09-STYLE-04: `f32::clamp` alone leaves `NaN` in the style state; the shared
                // policy (`NaN` -> opaque, `±∞` -> endpoints) keeps it in `0.0..=1.0`.
                sink.opacity(crate::style::normalized_opacity(val));
            }
            "font-size" => {
                let size = Self::cached_float(value)?;
                sink.font(|font| {
                    font.set_size(size);
                });
            }
            "font-weight" => {
                // G-404: listed as known with no arm behind it, so `font-weight: 700` was
                // accepted and silently discarded. Numeric weights and the named weights CSS
                // allows now reach `Font`'s own weight channel.
                let weight = Self::parse_font_weight(value)?;
                sink.font(|font| *font = font.with_weight_value(weight));
            }
            "font-family" => {
                let family = value.trim().trim_matches('"').trim_matches('\'').to_string();
                sink.font(|font| {
                    font.set_family(family.clone());
                });
            }
            // ── Gradient, shadow, touch target ──────────────────────────
            // `WidgetStyle` carries these three and CSS could not set any of them,
            // so a stylesheet could not express the whole style record. Covered here
            // so the CSS surface and the style type agree on what is settable.
            "background-gradient" => {
                sink.gradient(Self::cached_gradient(value)?);
            }
            "shadow" => {
                // `none` is a meaningful value, not an omission: it clears a shadow
                // the theme or a lower-priority sheet supplied.
                if value.trim().eq_ignore_ascii_case("none") {
                    sink.shadow(None);
                } else {
                    sink.shadow(Some(Self::cached_shadow(value)?));
                }
            }
            "touch-target" | "touch-target-size" => {
                // Order is `width height`, matching the two-value CSS shorthand the
                // spacing properties already use.
                let vals = Self::cached_lengths(value, 2)?;
                sink.touch_target(crate::core::Size::new(vals[0], vals[1]));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Parses `border-style` into the style primitive's own vocabulary (G-404).
    ///
    /// `none` is a value, not a missing declaration: it suppresses a border a lower layer set.
    /// An unsupported line style is refused rather than mapped onto `solid`, because painting a
    /// solid line for a requested dash is the kind of silent substitution this crate treats as a
    /// defect.
    fn parse_border_style(value: &str) -> Result<BorderStyle, String> {
        let text = value.trim();
        if text.eq_ignore_ascii_case("solid") {
            Ok(BorderStyle::Solid)
        } else if text.eq_ignore_ascii_case("dashed") {
            Ok(BorderStyle::Dashed)
        } else if text.eq_ignore_ascii_case("dotted") {
            Ok(BorderStyle::Dotted)
        } else if text.eq_ignore_ascii_case("none") {
            Ok(BorderStyle::None)
        } else {
            Err(format!(
                "unsupported border-style '{text}': expected `solid`, `dashed`, `dotted` or `none`"
            ))
        }
    }

    /// Parses `font-weight` into `Font`'s 100..=900 scale (G-404).
    ///
    /// A numeric value is rounded to the nearest hundred and clamped into the range `Font::is_valid`
    /// accepts, so `450` becomes `500` rather than a font the renderer refuses. The relative
    /// keywords are resolved against a regular base (`lighter` -> 300, `bolder` -> 700); a stylesheet
    /// has no parent weight to be relative to at parse time, and inventing one silently would be
    /// worse than a documented fixed step.
    fn parse_font_weight(value: &str) -> Result<u16, String> {
        let text = value.trim();
        let weight = if text.eq_ignore_ascii_case("normal") {
            400
        } else if text.eq_ignore_ascii_case("bold") {
            700
        } else if text.eq_ignore_ascii_case("lighter") {
            300
        } else if text.eq_ignore_ascii_case("bolder") {
            700
        } else {
            let numeric: u16 = text.parse().map_err(|_| {
                format!(
                    "invalid font-weight '{text}': expected 100..=900 or `normal`/`bold`/\
                     `lighter`/`bolder`"
                )
            })?;
            let rounded = ((u32::from(numeric) + 50) / 100) * 100;
            rounded.clamp(100, 900) as u16
        };
        Ok(weight)
    }

    /// Parse a CSS color value (#fff, #ffffff, rgb(r,g,b), rgba(r,g,b,a), named colors)
    pub fn parse_color(value: &str) -> Result<Color, String> {
        let v = value.trim();
        if let Some(hex) = v.strip_prefix('#') {
            Self::parse_hex_color(hex)
        } else if v.starts_with("rgb") {
            Self::parse_rgb_color(v)
        } else {
            // Named colors — common subset
            match v.to_lowercase().as_str() {
                "red" => Ok(Color::rgba(255, 0, 0, 255)),
                "green" | "lime" => Ok(Color::rgba(0, 255, 0, 255)),
                "blue" => Ok(Color::rgba(0, 0, 255, 255)),
                "white" => Ok(Color::rgba(255, 255, 255, 255)),
                "black" => Ok(Color::rgba(0, 0, 0, 255)),
                "gray" | "grey" => Ok(Color::rgba(128, 128, 128, 255)),
                "yellow" => Ok(Color::rgba(255, 255, 0, 255)),
                "cyan" | "aqua" => Ok(Color::rgba(0, 255, 255, 255)),
                "magenta" | "fuchsia" => Ok(Color::rgba(255, 0, 255, 255)),
                "orange" => Ok(Color::rgba(255, 165, 0, 255)),
                "purple" => Ok(Color::rgba(128, 0, 128, 255)),
                "navy" => Ok(Color::rgba(0, 0, 128, 255)),
                "teal" => Ok(Color::rgba(0, 128, 128, 255)),
                "maroon" => Ok(Color::rgba(128, 0, 0, 255)),
                "olive" => Ok(Color::rgba(128, 128, 0, 255)),
                "transparent" => Ok(Color::rgba(0, 0, 0, 0)),
                _ => Err(format!(
                    "unknown color '{v}'; use a #RGB/#RRGGBB/#RRGGBBAA literal or one of \
                     the named colors (black, white, red, green, blue, yellow, cyan, magenta, \
                     gray, orange, purple, pink, brown, navy, teal, maroon, olive, transparent)"
                )),
            }
        }
    }

    fn parse_hex_color(hex: &str) -> Result<Color, String> {
        let hex = hex.trim();
        // `len()` and the slices below are byte-based, so a non-ASCII character would panic on a
        // char-boundary split (`"#日本"` is 6 bytes and enters the 6-digit arm). Reject non-hex
        // input up front and let the normal "invalid colour" path report it.
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("Invalid hex: {hex}"));
        }
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                let g = u8::from_str_radix(&hex[1..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                let b = u8::from_str_radix(&hex[2..3], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                (r, g, b, 255)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                (r, g, b, a)
            }
            _ => return Err(format!("Invalid hex color length: #{hex}")),
        };
        Ok(Color::rgba(r, g, b, a))
    }

    fn parse_rgb_color(v: &str) -> Result<Color, String> {
        let inner = v.trim_start_matches("rgba").trim_start_matches("rgb");
        let inner = inner.trim_start_matches('(').trim_end_matches(')').trim();
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        match parts.len() {
            3 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                Ok(Color::rgba(r, g, b, 255))
            }
            4 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let a: f32 = parts[3].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                Ok(Color::rgba(r, g, b, (a * 255.0).round() as u8))
            }
            _ => Err(format!("Invalid rgb(a) format: {v}")),
        }
    }

    /// Parse a CSS length value (e.g., "10px", "2", "0")
    fn parse_length(value: &str) -> Result<u32, String> {
        let v = value.trim();
        let num_str = v.trim_end_matches("px").trim_end_matches("pt").trim_end_matches("em").trim();
        let f: f32 = num_str.parse().map_err(|_| format!("Invalid length: {value}"))?;
        Ok(f.max(0.0) as u32)
    }

    fn parse_float(value: &str) -> Result<f32, String> {
        let v = value.trim().trim_end_matches("px").trim_end_matches("pt").trim();
        v.parse::<f32>().map_err(|_| format!("Invalid number: {value}"))
    }

    /// Parse 1-4 space-separated CSS values into exactly N values (cloned if fewer).
    /// E.g. "10px 20px" with count=4 returns [10, 20, 10, 20] (CSS trbl pattern).
    fn parse_space_separated_lengths(value: &str, count: usize) -> Result<Vec<u32>, String> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        let parsed: Vec<u32> =
            parts.iter().map(|p| Self::parse_length(p)).collect::<Result<Vec<_>, _>>()?;
        match parsed.len() {
            1 => Ok(vec![parsed[0]; count]),
            2 if count == 2 => Ok(parsed),
            2 => Ok(vec![parsed[0], parsed[1], parsed[0], parsed[1]]),
            3 => Ok(vec![parsed[0], parsed[1], parsed[2], parsed[1]]),
            4 => Ok(parsed),
            _ => Err(format!("Expected 1-4 values for spacing, got {}", parts.len())),
        }
    }

    /// Parse a gradient value.
    ///
    /// The accepted syntax is a deliberately small subset of CSS's:
    ///
    /// ```text
    /// linear(<angle>deg, <color> [<position>], <color> [<position>], ...)
    /// ```
    ///
    /// Only the **linear** geometry is supported, because that is the one the
    /// render layer can paint without a centre or radius to resolve: a radial or
    /// conic ramp needs geometry CSS does not carry. Rejecting them by name is
    /// better than accepting the syntax and silently painting a linear ramp.
    ///
    /// A stop without an explicit position is distributed evenly, matching how a
    /// caller would read the declaration.
    fn parse_gradient(value: &str) -> Result<crate::style::Gradient, String> {
        let v = value.trim();
        if v.eq_ignore_ascii_case("none") {
            return Err(
                "'none' is not a gradient; omit the declaration to leave a background colour, or \
                 write `background-color: transparent`"
                    .to_string(),
            );
        }

        let inner = v
            .strip_prefix("linear-gradient(")
            .or_else(|| v.strip_prefix("linear("))
            .and_then(|rest| rest.strip_suffix(')'))
            .ok_or_else(|| {
                format!(
                    "unsupported gradient '{v}': only `linear-gradient(<angle>deg, <color> [<pos>], \
                     ...)` is supported (radial and conic ramps need geometry the theme schema does \
                     not carry)"
                )
            })?;

        let mut parts = inner.split(',').map(str::trim).filter(|p| !p.is_empty());

        // Optional leading angle. `to bottom` and friends are not supported: they
        // describe a box-relative direction the widget tree does not resolve here.
        let mut angle = 180.0_f32;
        let mut stops: Vec<crate::style::GradientStop> = Vec::new();
        let mut pending: Vec<(crate::core::Color, Option<f32>)> = Vec::new();

        for part in parts.by_ref() {
            if let Some(deg) = part.strip_suffix("deg") {
                angle = deg.trim().parse::<f32>().map_err(|_| {
                    format!("invalid gradient angle in '{v}': '{part}' is not a number of degrees")
                })?;
                continue;
            }
            // `<color> [<position>]`
            let mut tokens = part.split_whitespace();
            let color_token =
                tokens.next().ok_or_else(|| format!("gradient stop in '{v}' has no colour"))?;
            let color = Self::parse_color(color_token)?;
            let position = match tokens.next() {
                Some(raw) => Some(Self::parse_stop_position(raw)?),
                None => None,
            };
            pending.push((color, position));
        }

        if pending.len() < 2 {
            return Err(format!(
                "gradient '{v}' needs at least two colour stops, found {}",
                pending.len()
            ));
        }

        // Distribute the stops that did not state a position evenly between the
        // fixed ones, which is how the ramp in a declaration is read.
        let last = pending.len() - 1;
        for (index, (color, position)) in pending.into_iter().enumerate() {
            let resolved = position.unwrap_or(index as f32 / last as f32);
            stops.push(crate::style::GradientStop::new(resolved, color));
        }

        Ok(crate::style::Gradient::linear(
            crate::core::Point::new(0, 0),
            crate::core::Point::new(0, 0),
        )
        .with_stops(stops)
        .with_angle(angle))
    }

    /// Parse a gradient stop position: a fraction (`0.5`) or a percentage (`50%`).
    fn parse_stop_position(value: &str) -> Result<f32, String> {
        let v = value.trim();
        let fraction = if let Some(percent) = v.strip_suffix('%') {
            percent.trim().parse::<f32>().map_err(|_| {
                format!("invalid gradient stop position '{value}': not a percentage")
            })? / 100.0
        } else {
            v.parse::<f32>().map_err(|_| {
                format!("invalid gradient stop position '{value}': not a number or percentage")
            })?
        };
        Ok(fraction.clamp(0.0, 1.0))
    }

    /// Parse a shadow value: `<offset-x> <offset-y> <blur> <color>`.
    ///
    /// The four parts are required, because a shadow with a missing colour would
    /// have to invent one and a shadow with a missing extent is not a shadow. The
    /// colour may be written first or last, matching the two orders CSS accepts.
    fn parse_shadow(value: &str) -> Result<crate::style::Shadow, String> {
        let v = value.trim();
        let parts: Vec<&str> = v.split_whitespace().collect();
        if parts.len() != 4 {
            return Err(format!(
                "shadow '{v}' must be `<offset-x> <offset-y> <blur> <color>`, found {} part(s)",
                parts.len()
            ));
        }

        // The colour may lead or trail; the three lengths keep their relative order.
        let (color, lengths): (crate::core::Color, &[&str]) =
            if let Ok(color) = Self::parse_color(parts[3]) {
                (color, &parts[..3])
            } else if let Ok(color) = Self::parse_color(parts[0]) {
                (color, &parts[1..])
            } else {
                return Err(format!(
                    "shadow '{v}' has no recognisable colour; expected a #RGB/#RRGGBB/#RRGGBBAA \
                     literal or a named colour"
                ));
            };

        let offset_x = Self::parse_signed_length(lengths[0])?;
        let offset_y = Self::parse_signed_length(lengths[1])?;
        let blur = Self::parse_length(lengths[2])?;
        Ok(crate::style::Shadow { x: offset_x, y: offset_y, blur, color })
    }

    /// Parse a length that may be negative.
    ///
    /// [`Self::parse_length`] clamps to `>= 0`, which is right for a size and wrong
    /// for a shadow offset: `-2px 2px` is a perfectly ordinary shadow, and clamping
    /// it would silently move the shadow instead of reporting the input.
    fn parse_signed_length(value: &str) -> Result<i32, String> {
        let v = value.trim();
        let num_str = v.trim_end_matches("px").trim_end_matches("pt").trim_end_matches("em").trim();
        let f: f32 = num_str.parse().map_err(|_| format!("Invalid length: {value}"))?;
        Ok(f.round() as i32)
    }

    fn skip_whitespace_and_comments(chars: &[char], mut pos: usize) -> usize {
        while pos < chars.len() {
            // Skip multi-line comment
            if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
                pos += 2;
                while pos + 1 < chars.len() && !(chars[pos] == '*' && chars[pos + 1] == '/') {
                    pos += 1;
                }
                pos += 2; // skip */
                continue;
            }
            // Skip single-line comment
            if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '/' {
                pos += 2;
                while pos < chars.len() && chars[pos] != '\n' {
                    pos += 1;
                }
                pos += 1;
                continue;
            }
            if !chars[pos].is_whitespace() {
                break;
            }
            pos += 1;
        }
        pos
    }
}

impl CssParser {
    /// Parse CSS text into StyleSheet with stored declarations.
    /// Then use `CssParser::parse_and_apply` (or `StyleSheetManager::apply_to`) to
    /// apply the matching rules to a WidgetStyle.
    pub fn parse_and_apply(
        css: &str,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        let rules = Self::parse_rules(css)?;
        // Filter and apply matching rules in order
        for rule in &rules {
            if let Some(selector) = Self::parse_selector(&rule.selector_text) {
                if selector.matches(kind, class, id, state) {
                    Self::apply_declarations(&rule.declarations, style)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rule whose kind is unknown must match nothing. A **pseudo-class** that does not parse
    /// used to be dropped instead, leaving the rest of the selector intact, so `Button:hoover`
    /// selected every button in every state.
    #[test]
    fn an_unknown_pseudo_class_makes_the_selector_match_nothing() {
        assert!(CssParser::parse_selector("Button:hoover").is_none());
        assert!(CssParser::parse_selector("Button:focus-within").is_none());
        // Known states still parse.
        assert!(CssParser::parse_selector("Button:hover").is_some());
    }

    /// Non-ASCII input must be reported, not panic on a char-boundary slice.
    #[test]
    fn a_non_ascii_hex_colour_is_rejected_without_panicking() {
        for value in ["#日本", "#日aaaaa", "#zzzzzz", "#fffz"] {
            assert!(CssParser::parse_color(value).is_err(), "{value} must be rejected");
        }
    }

    /// A class selector must outrank a kind selector regardless of source order.
    #[test]
    fn specificity_ranks_a_class_above_a_kind() {
        let kind = CssParser::parse_selector("Button").expect("kind selector");
        let class = CssParser::parse_selector("Button.primary").expect("class selector");
        let id = CssParser::parse_selector("#ok").expect("id selector");
        assert!(kind.specificity() < class.specificity());
        assert!(class.specificity() < id.specificity());
    }

    #[test]
    fn test_parse_simple_rule() {
        let css = "Button { color: #333; font-size: 14px; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector_text, "Button");
        assert_eq!(rules[0].declarations.len(), 2);
    }

    #[test]
    fn test_parse_class_selector() {
        let css = ".primary { background: #0066cc; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, ".primary");
    }

    #[test]
    fn test_parse_id_selector() {
        let css = "#submit { color: red; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, "#submit");
    }

    #[test]
    fn test_parse_pseudo_selector() {
        let css = "Button:hover { background: blue; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, "Button:hover");
    }

    /// The three validation states parse, and they alias their natural spellings.
    ///
    /// # The gap this pins
    ///
    /// `WidgetState::Error`/`Warning`/`Success` and the theme's `"<kind>:error"` override keys have
    /// existed for as long as those states have. The CSS path understood six pseudo-classes and
    /// none of the three, so a sheet targeting `line_edit:error` could not be written — and inline
    /// validation is the most common state a form has. The assertion is the parsed selector, so a
    /// name that stops parsing fails rather than silently matching nothing.
    #[test]
    fn the_validation_states_parse_as_pseudo_classes() {
        for (name, expected) in [
            ("error", PseudoState::Error),
            ("invalid", PseudoState::Error),
            ("warning", PseudoState::Warning),
            ("success", PseudoState::Success),
            ("valid", PseudoState::Success),
        ] {
            assert_eq!(
                CssParser::parse_pseudo_state(name),
                Some(expected),
                "`{name}` must resolve to the state the theme's `\"<kind>:<state>\"` keys use"
            );
            // And a full rule carrying it parses, so the alias is reachable from source text and
            // not merely from the helper.
            let css = format!("LineEdit:{name} {{ background: blue; }}");
            let rules = CssParser::parse_rules(&css)
                .unwrap_or_else(|e| panic!("LineEdit:{name} failed to parse: {e:?}"));
            assert_eq!(rules.len(), 1, "LineEdit:{name}");
            assert_eq!(rules[0].selector_text, format!("LineEdit:{name}"));
        }
    }

    /// An unknown pseudo-class is not silently normal.
    ///
    /// The negative half is what separates "the state parsed" from "the state means something":
    /// a parser that returned `Some(PseudoState::Normal)` for an unrecognised name would satisfy
    /// the test above and make a typo'd `:erorr` paint every resting control.
    #[test]
    fn an_unknown_pseudo_class_is_rejected_rather_than_normal() {
        assert_eq!(CssParser::parse_pseudo_state("erorr"), None);
        assert_eq!(CssParser::parse_pseudo_state("normal"), Some(PseudoState::Normal));
        assert_ne!(CssParser::parse_pseudo_state("erorr"), Some(PseudoState::Normal));
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = r#"
            Button { color: black; }
            Label { color: gray; font-size: 12px; }
            .warn { color: red; }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn test_parse_hex_color_3() {
        let c = CssParser::parse_color("#f00").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_6() {
        let c = CssParser::parse_color("#ff0000").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_8() {
        let c = CssParser::parse_color("#ff000080").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn test_parse_rgb_color() {
        let c = CssParser::parse_color("rgb(255, 0, 0)").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_rgba_color() {
        let c = CssParser::parse_color("rgba(255, 0, 0, 0.5)").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn test_parse_named_colors() {
        let c = CssParser::parse_color("red").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
        let c = CssParser::parse_color("white").unwrap();
        assert_eq!(c, Color::rgba(255, 255, 255, 255));
        let c = CssParser::parse_color("transparent").unwrap();
        assert_eq!(c, Color::rgba(0, 0, 0, 0));
    }

    #[test]
    fn test_parse_length() {
        assert_eq!(CssParser::parse_length("10px").unwrap(), 10);
        assert_eq!(CssParser::parse_length("0").unwrap(), 0);
        assert_eq!(CssParser::parse_length("5.5px").unwrap(), 5);
    }

    #[test]
    fn test_parse_spacing_shorthand_1() {
        let vals = CssParser::parse_space_separated_lengths("10px", 4).unwrap();
        assert_eq!(vals, vec![10, 10, 10, 10]);
    }

    #[test]
    fn test_parse_spacing_shorthand_2() {
        let vals = CssParser::parse_space_separated_lengths("10px 20px", 4).unwrap();
        assert_eq!(vals, vec![10, 20, 10, 20]);
    }

    #[test]
    fn test_parse_spacing_shorthand_4() {
        let vals = CssParser::parse_space_separated_lengths("1px 2px 3px 4px", 4).unwrap();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    /// The same declaration text is converted once, and an inline list is split once (G-21).
    #[test]
    fn declaration_values_are_converted_once_per_distinct_text() {
        DECL_CACHE.with(|cache| cache.borrow_mut().clear());
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations_text("background: #123456; padding: 4px 8px", &mut style)
            .expect("valid inline CSS");
        let first = DECL_CACHE.with(|cache| cache.borrow().len());
        CssParser::apply_declarations_text("background: #123456; padding: 4px 8px", &mut style)
            .expect("same inline CSS");
        assert_eq!(
            DECL_CACHE.with(|cache| cache.borrow().len()),
            first,
            "a repeated apply must reuse the converted values"
        );
        assert_eq!(style.background_color, Some(Color::rgba(0x12, 0x34, 0x56, 255)));
    }

    #[test]
    fn test_apply_color_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "color".into(), value: "#333".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.text_color, Some(Color::rgba(51, 51, 51, 255)));
    }

    #[test]
    fn test_apply_background_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "background".into(), value: "#0066cc".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.background_color, Some(Color::rgba(0, 102, 204, 255)));
    }

    #[test]
    fn test_parse_complex_stylesheet() {
        let css = r#"
            /* Button base style */
            Button {
                color: #333;
                font-size: 14px;
                padding: 8px 16px;
                border-radius: 4px;
            }
            /* Primary variant */
            .primary {
                background: #0066cc;
                color: white;
            }
            /* Hover state */
            Button:hover {
                background: #0052a3;
            }
            /* Disabled state */
            Button:disabled {
                opacity: 0.5;
            }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 4);

        // Test applying rules to a Button with .primary class
        let mut style = WidgetStyle::default();
        for rule in &rules {
            if let Some(sel) = CssParser::parse_selector(&rule.selector_text) {
                if sel.matches("Button", Some("primary"), None, None) {
                    CssParser::apply_declarations(&rule.declarations, &mut style).unwrap();
                }
            }
        }
        // Button base applied
        assert_eq!(style.border_radius, Some(4));
        // .primary overrides
        assert_eq!(style.background_color, Some(Color::rgba(0, 102, 204, 255)));
    }

    #[test]
    fn test_selector_parser() {
        let sel = CssParser::parse_selector("Button").unwrap();
        assert_eq!(sel, CssSelector::Kind("Button".to_string()));

        let sel = CssParser::parse_selector(".primary").unwrap();
        assert_eq!(sel, CssSelector::Class("primary".to_string()));

        let sel = CssParser::parse_selector("#main").unwrap();
        assert_eq!(sel, CssSelector::Id("main".to_string()));

        let sel = CssParser::parse_selector(":hover").unwrap();
        assert_eq!(sel, CssSelector::State(PseudoState::Hover));
    }

    #[test]
    fn test_parse_empty_returns_no_rules() {
        let rules = CssParser::parse_rules("").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_error_unclosed_brace() {
        let result = CssParser::parse_rules("Button { color: red; ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_comments_are_ignored() {
        let css = r#"
            /* this is a comment */
            Button { color: red; }
            /* another comment */
            Label { color: blue; }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_opacity_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "opacity".into(), value: "0.5".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert!((style.opacity.unwrap() - 0.5).abs() < 0.01);
    }

    /// D09-STYLE-04: CSS `opacity` used to `clamp` and leave `NaN` in the style state. The
    /// declaration must now normalise non-finite input the same way the builder and theme
    /// token paths do.
    #[test]
    fn opacity_declaration_normalizes_non_finite_values() {
        let mut style = WidgetStyle::default();
        CssParser::apply_declaration_text("opacity: NaN", &mut style).expect("NaN parses as f32");
        assert_eq!(style.opacity, Some(1.0), "NaN must normalise to opaque, not stay NaN");

        let mut style = WidgetStyle::default();
        CssParser::apply_declaration_text("opacity: inf", &mut style).expect("+inf parses as f32");
        assert_eq!(style.opacity, Some(1.0));

        let mut style = WidgetStyle::default();
        CssParser::apply_declaration_text("opacity: -inf", &mut style).expect("-inf parses as f32");
        assert_eq!(style.opacity, Some(0.0));

        let mut style = WidgetStyle::default();
        CssParser::apply_declaration_text("opacity: 2", &mut style).expect("finite out of range");
        assert_eq!(style.opacity, Some(1.0));
    }

    #[test]
    fn test_margin_shorthand() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "margin".into(), value: "10px 20px".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.margin.top, 10);
        assert_eq!(style.margin.right, 20);
        assert_eq!(style.margin.bottom, 10);
        assert_eq!(style.margin.left, 20);
    }

    // ── Unknown selector kinds ────────────────────────────────────────────

    /// A selector naming an unknown widget kind resolves to no selector, rather
    /// than silently becoming `Window`.
    #[test]
    fn unknown_kind_selects_nothing() {
        let selector = CssParser::parse_selector("NotAWidget").expect("the text parses");
        assert_eq!(selector.to_selector(), None);
    }

    /// A **known** kind still resolves, so the `None` above is about the unknown
    /// name and not a blanket refusal.
    #[test]
    fn known_kind_still_resolves() {
        let selector = CssParser::parse_selector("Button").expect("the text parses");
        assert_eq!(selector.to_selector(), Some(Selector::Kind(WidgetKind::Button)));
    }

    /// `widget_kind_from_str` must accept every spelling `CssSelector::matches` does.
    ///
    /// # Why this test exists
    ///
    /// The two CSS entry points disagreed about a kind name. `matches` compared strings
    /// case-insensitively, while `widget_kind_from_str` (reached through
    /// `CssSelector::to_selector`, which is what `RuleSet::parse` and
    /// `StyleSheetManager::apply_to` use) had a hand-written table with only three
    /// multi-word kinds in it — `MenuBar`, `ToolBar` and `StatusBar` were absent, so
    /// `menu_bar { … }` resolved to `None` on that path and silently styled nothing.
    ///
    /// This test pins the invariant the module doc claims: every spelling that reaches a
    /// kind through `matches` must also resolve through `widget_kind_from_str`.
    ///
    /// # Why the cases are gated
    ///
    /// Several of these variants exist only where `widgets_unstripped` is on (see
    /// `kind.rs`), so naming them unconditionally would break `mini`/`embedded`. Each row
    /// carries the same gate as its declaration, and the always-available rows stay
    /// ungated so every profile exercises the table.
    #[test]
    fn every_kind_selector_spelling_resolves() {
        // The `Debug` spelling a widget reports, and the factory spellings an author may
        // reasonably write for it.
        // `mut` is needed only where the `extend_from_slice` block below is compiled in
        // (`widgets_unstripped`); a stripped profile builds the same list without
        // extending it, hence the allow.
        #[allow(unused_mut)]
        let mut cases: alloc::vec::Vec<(&str, WidgetKind)> = alloc::vec::Vec::from([
            ("Button", WidgetKind::Button),
            ("button", WidgetKind::Button),
            ("CheckBox", WidgetKind::CheckBox),
            ("RadioButton", WidgetKind::RadioButton),
            ("LineEdit", WidgetKind::LineEdit),
            ("ScrollArea", WidgetKind::ScrollArea),
            ("ScrollBar", WidgetKind::ScrollBar),
            ("ProgressBar", WidgetKind::ProgressBar),
            ("GroupBox", WidgetKind::GroupBox),
            ("ImageView", WidgetKind::ImageView),
            ("ListBox", WidgetKind::ListBox),
            ("SpinBox", WidgetKind::SpinBox),
            ("ComboBox", WidgetKind::ComboBox),
        ]);
        #[cfg(widgets_unstripped)]
        {
            cases.extend_from_slice(&[
                ("MenuBar", WidgetKind::MenuBar),
                ("menu_bar", WidgetKind::MenuBar),
                ("menubar", WidgetKind::MenuBar),
                ("ToolBar", WidgetKind::ToolBar),
                ("tool_bar", WidgetKind::ToolBar),
                ("StatusBar", WidgetKind::StatusBar),
                ("status_bar", WidgetKind::StatusBar),
                ("TabWidget", WidgetKind::TabWidget),
                ("MenuButton", WidgetKind::MenuButton),
                ("Splitter", WidgetKind::Splitter),
            ]);
        }

        for (spelling, expected) in cases {
            assert_eq!(
                widget_kind_from_str(spelling),
                Some(expected),
                "{spelling:?} must resolve to {expected:?}"
            );
        }

        // And a compound/unknown name is still refused rather than guessed at.
        assert_eq!(widget_kind_from_str("NotAWidget"), None);
    }

    /// A kind selector matches the spellings the rest of the library publishes.
    ///
    /// # Why this test exists
    ///
    /// `CssSelector::matches` used to compare with `eq_ignore_ascii_case` against
    /// the widget's `Debug` spelling. That bridges case but **not** separators, so
    /// `menu_bar { … }` — the canonical factory name, and the spelling used by the
    /// capability registry and the JSON loader — matched nothing, while `menubar`
    /// worked. Nothing was logged: the rule parsed fine, so the "unknown kind"
    /// diagnostic never ran, and the user simply saw their stylesheet ignored.
    #[test]
    fn a_kind_selector_accepts_the_same_spellings_the_factory_does() {
        let widget_kind = "MenuBar";
        for spelling in ["MenuBar", "menubar", "menu_bar", "MENU-BAR"] {
            let selector = CssSelector::Kind(spelling.to_string());
            assert!(
                selector.matches(widget_kind, None, None, None),
                "selector {spelling:?} must match the widget kind {widget_kind:?}"
            );
        }
        // A different kind must still not match, so the fix is not a blanket accept.
        assert!(!CssSelector::Kind("menu".to_string()).matches(widget_kind, None, None, None));
        assert!(!CssSelector::Kind("tool_bar".to_string()).matches(widget_kind, None, None, None));
    }

    /// The separator-insensitive match must reach the applied style, not just the
    /// predicate — this is the end-to-end path a stylesheet actually takes.
    #[test]
    fn a_separator_spelled_kind_selector_applies_its_rule() {
        for spelling in ["menu_bar", "MenuBar", "menubar"] {
            let css = format!("{spelling} {{ background-color: #ff0000; }}");
            let mut style = WidgetStyle::default();
            CssParser::parse_and_apply(&css, "MenuBar", None, None, None, &mut style)
                .expect("the rule must parse");
            assert_eq!(
                style.background_color,
                Some(Color::rgb(255, 0, 0)),
                "the {spelling:?} rule must apply to a MenuBar"
            );
        }
    }

    /// The regression this guards: before the fix, `Stepper { … }` resolved to
    /// `WidgetKind::Window` and therefore styled windows. Now it resolves to
    /// nothing, so it cannot match a window.
    #[test]
    fn an_unknown_kind_never_becomes_a_window() {
        let selector = CssParser::parse_selector("Stepper").expect("the text parses");
        assert_ne!(
            selector.to_selector(),
            Some(Selector::Kind(WidgetKind::Window)),
            "an unknown name must not be reinterpreted as Window"
        );
    }

    /// A compound selector with one unknown part resolves to nothing: a rule whose
    /// subject does not exist cannot be made to apply by adding a class to it.
    #[test]
    fn a_compound_selector_with_an_unknown_kind_selects_nothing() {
        let selector = CssParser::parse_selector("NotAWidget.primary").expect("the text parses");
        assert_eq!(selector.to_selector(), None);
    }

    /// `CssParser::parse` skips a rule whose kind is unknown instead of registering
    /// it against the wrong kind. The known rule beside it still lands.
    #[test]
    fn parse_skips_only_the_rule_with_an_unknown_kind() {
        let css = "Button { color: #111; }\nStepper { color: #222; }";
        let sheet = CssParser::parse(css).expect("the sheet parses");
        let kinds: Vec<_> = sheet.rules().iter().map(|rule| rule.selector.clone()).collect();
        assert_eq!(kinds, vec![Selector::Kind(WidgetKind::Button)], "only the known rule lands");
    }

    /// Every field of `WidgetStyle` that a stylesheet should be able to set is
    /// settable. `background_gradient`, `shadow` and `touch_target` were defined on
    /// the style record but unreachable from CSS, so a stylesheet could not express
    /// the whole style. This test is the guard against a future field being added
    /// without a property.
    #[test]
    fn the_style_fields_a_stylesheet_should_set_are_settable() {
        let declarations = [
            CssDeclaration {
                property: "background-gradient".into(),
                value: "linear-gradient(90deg, #ff0000, #0000ff)".into(),
            },
            CssDeclaration { property: "shadow".into(), value: "2px 3px 4px #112233".into() },
            CssDeclaration { property: "touch-target".into(), value: "44px 44px".into() },
        ];
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&declarations, &mut style).expect("all three must apply");

        let gradient = style.background_gradient.expect("a gradient must be set");
        assert_eq!(gradient.stops.len(), 2, "two stops, evenly distributed");
        assert_eq!(gradient.stops[0].color, Color::rgba(255, 0, 0, 255));
        assert_eq!(gradient.stops[1].color, Color::rgba(0, 0, 255, 255));
        assert_eq!(gradient.stops[0].position, 0.0);
        assert_eq!(gradient.stops[1].position, 1.0);

        let shadow = style.shadow.expect("a shadow must be set");
        assert_eq!((shadow.x, shadow.y, shadow.blur), (2, 3, 4));
        assert_eq!(shadow.color, Color::rgba(0x11, 0x22, 0x33, 255));

        assert_eq!(style.touch_target, Some(crate::core::Size::new(44, 44)));
    }

    /// A gradient stop may carry an explicit position, as a fraction or a percentage.
    #[test]
    fn gradient_stop_positions_are_honoured() {
        let decl = CssDeclaration {
            property: "background-gradient".into(),
            value: "linear-gradient(#ff0000 0%, #00ff00 25%, #0000ff)".into(),
        };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");

        let stops = style.background_gradient.expect("gradient").stops;
        assert_eq!(stops.len(), 3);
        assert_eq!(stops[0].position, 0.0);
        assert_eq!(stops[1].position, 0.25);
        // A stop with no position takes the evenly-spaced slot, which for the last
        // of three stops is 1.0.
        assert_eq!(stops[2].position, 1.0);
    }

    /// `shadow: none` clears a shadow rather than being rejected, so a higher-priority
    /// sheet can remove one a lower-priority sheet installed.
    #[test]
    fn shadow_none_clears_a_shadow() {
        let mut style = WidgetStyle::default();
        let set = CssDeclaration { property: "shadow".into(), value: "1px 1px 2px #000000".into() };
        CssParser::apply_declarations(&[set], &mut style).expect("apply");
        assert!(style.shadow.is_some());

        let clear = CssDeclaration { property: "shadow".into(), value: "none".into() };
        CssParser::apply_declarations(&[clear], &mut style).expect("apply");
        assert_eq!(style.shadow, None, "`none` must clear the shadow");
    }

    /// A negative shadow offset is legal and must survive, not be clamped to zero.
    #[test]
    fn a_negative_shadow_offset_is_preserved() {
        let decl =
            CssDeclaration { property: "shadow".into(), value: "-2px 2px 4px #000000".into() };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");
        let shadow = style.shadow.expect("shadow");
        assert_eq!(shadow.x, -2, "a negative offset must not be clamped to 0");
        assert_eq!(shadow.y, 2);
    }

    /// A shadow written with the colour first is accepted too; CSS allows both orders.
    #[test]
    fn a_shadow_may_lead_with_its_colour() {
        let decl =
            CssDeclaration { property: "shadow".into(), value: "#000000 1px 2px 3px".into() };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");
        let shadow = style.shadow.expect("shadow");
        assert_eq!((shadow.x, shadow.y, shadow.blur), (1, 2, 3));
        assert_eq!(shadow.color, Color::rgba(0, 0, 0, 255));
    }

    /// An authored `shadow:` reaches the face the renderer actually paints.
    ///
    /// The property used to be parsed into `WidgetStyle::shadow`, inherited by "`merge`/`inherit`,
    /// and then read by no renderer at all — the paint path takes its shadow from
    /// `SurfaceStyle`, which knew shadows only through its elevation ladder. So `shadow: 0 2 6 black`
    /// was a documented CSS property with no effect whatsoever.
    ///
    /// This asserts the whole chain rather than the parse: declaration → style → the surface →
    /// what `SurfaceStyle::shadow()` hands the painter. The last step is the one that was missing,
    /// so asserting only `style.shadow` (which the tests above already did) would have kept
    /// passing while nothing was drawn.
    #[test]
    fn an_authored_shadow_reaches_the_painted_surface() {
        let decl =
            CssDeclaration { property: "shadow".into(), value: "3px 4px 6px #000080".into() };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");

        let surface = style.surface.expect("applying a shadow must leave a surface behind");
        let painted = surface
            .shadow(Color::BLACK)
            .expect("the authored shadow must be the one the renderer asks for");
        assert_eq!(
            (painted.x, painted.y, painted.blur),
            (3, 4, 6),
            "the four authored numbers must survive to the render layer: {painted:?}"
        );
        assert_eq!(
            painted.color,
            Color::rgba(0, 0, 128, 255),
            "including the colour, which the elevation ladder could not have expressed"
        );
    }

    /// `shadow: none` clears the override and leaves the surface's own shadow in place.
    #[test]
    fn clearing_a_shadow_leaves_the_surfaces_own_shadow_alone() {
        let mut style = WidgetStyle::default();
        CssParser::apply_declaration_text("shadow: 1px 1px 2px #000000", &mut style)
            .expect("apply");
        CssParser::apply_declaration_text("shadow: none", &mut style).expect("clear");
        assert!(style.shadow.is_none(), "`none` clears the authored value");
        let surface = style.surface.expect("the surface survives");
        assert!(
            surface.custom_shadow.is_none(),
            "and the override goes with it, so the level's own shadow applies"
        );
    }

    /// Malformed values are reported rather than silently ignored.
    #[test]
    fn malformed_gradient_and_shadow_values_are_reported() {
        let cases = [
            ("background-gradient", "radial-gradient(#ff0000, #0000ff)"),
            ("background-gradient", "linear-gradient(#ff0000)"),
            ("background-gradient", "not a gradient"),
            ("shadow", "1px 2px #000000"),
            ("shadow", "1px 2px 3px not-a-colour"),
        ];
        for (property, value) in cases {
            let decl = CssDeclaration { property: property.into(), value: value.into() };
            let mut style = WidgetStyle::default();
            assert!(
                CssParser::apply_declarations(&[decl], &mut style).is_err(),
                "{property}: {value:?} should be rejected, not silently ignored"
            );
        }
    }

    /// An unsupported geometry is refused *by name*, so a caller using a radial ramp
    /// is told why rather than getting a linear one painted instead.
    #[test]
    fn an_unsupported_gradient_geometry_says_so() {
        let decl = CssDeclaration {
            property: "background-gradient".into(),
            value: "radial-gradient(#ff0000, #0000ff)".into(),
        };
        let mut style = WidgetStyle::default();
        let error = CssParser::apply_declarations(&[decl], &mut style).expect_err("must reject");
        assert!(error.contains("linear-gradient"), "{error}");
        assert!(
            error.contains("radial") || error.contains("geometry"),
            "the message must name the unsupported geometry: {error}"
        );
    }

    /// `is_known_property` must accept exactly what `apply_value` acts on, and reject
    /// everything else.
    ///
    /// # Why this test is the point of the two lists
    ///
    /// `apply_value` silently ignores an unknown property, so a name the list claims but the apply
    /// does not handle would be reported as applied while changing nothing — the exact failure mode
    /// the list exists to prevent. The loop below walks [`CssParser::known_properties`] itself,
    /// not a hand-copied subset: the previous version named twelve of the eighteen entries and
    /// omitted precisely the two that had no implementation (`border-style`, `font-weight`), so it
    /// stayed green while the trace called them known and the apply dropped them (G-404).
    #[test]
    fn known_property_list_matches_what_apply_one_handles() {
        for property in CssParser::known_properties() {
            assert!(CssParser::is_known_property(property), "{property} should be known");
            let before = WidgetStyle::default();
            let mut after = before.clone();
            let value = sample_value(property);
            CssParser::apply_declaration_text(&format!("{property}: {value}"), &mut after)
                .unwrap_or_else(|error| panic!("{property} should parse, got: {error}"));
            assert_ne!(
                format!("{after:?}"),
                format!("{before:?}"),
                "{property} is listed as known but applying it changed nothing"
            );
            // The trace's judgement, through the pure-parse wrapper, must agree with the apply.
            let declaration =
                CssDeclaration { property: String::from(*property), value: String::from(value) };
            assert!(
                CssParser::validate_declaration(&declaration).is_ok(),
                "{property} must validate the same value it applies"
            );
        }

        // `boundary` names are case-insensitive in CSS and the apply agrees with the list.
        assert!(CssParser::is_known_property("Color"));
        let mut cased = WidgetStyle::default();
        CssParser::apply_declaration_text("Color: #123456", &mut cased)
            .expect("a case variant is a declaration like any other");
        assert_eq!(cased.text_color, Some(Color::rgba(0x12, 0x34, 0x56, 255)));

        // A misspelling must be rejected rather than reported as applied.
        assert!(!CssParser::is_known_property("backgrond-color"));
        let mut style = WidgetStyle::default();
        let error = CssParser::apply_declaration_text("not-a-declaration", &mut style)
            .expect_err("a declaration with no ':' must be refused");
        assert!(error.contains("':'"), "the error should name the missing separator: {error}");

        let error = CssParser::apply_declaration_text("background-color:", &mut style)
            .expect_err("an empty value must be refused");
        assert!(error.contains("empty value"), "the error should say the value is empty: {error}");

        assert!(CssParser::apply_declaration_text("  ", &mut style).is_err());
    }

    /// A value that parses for each property the parser knows, used by the behavioural lock above.
    fn sample_value(property: &str) -> &'static str {
        match property {
            "color" | "text-color" | "foreground" | "background" | "background-color"
            | "border-color" => "#123456",
            "border-width" | "border-radius" | "padding-top" | "padding-right"
            | "padding-bottom" | "padding-left" | "margin-top" | "margin-right"
            | "margin-bottom" | "margin-left" => "2",
            "border-style" => "dashed",
            "font-size" => "16",
            "font-weight" => "700",
            "font-family" => "Inter",
            "padding" | "margin" | "touch-target" | "touch-target-size" => "2 2",
            "opacity" => "0.5",
            "background-gradient" => "linear-gradient(90deg, #FFF, #000)",
            "shadow" => "0 1 2 #000",
            other => panic!("the lock needs a sample value for `{other}`"),
        }
    }

    // ── Per-corner radii (STY-01) ────────────────────────────────────────

    /// The CSS 1–4 value shorthand maps onto the four corners in the same order the `padding`
    /// and `margin` shorthands use, and one value still means "all four".
    #[test]
    fn border_radius_shorthand_maps_one_to_four_values() {
        let apply = |value: &str| {
            let mut style = WidgetStyle::default();
            CssParser::apply_declaration_text(&format!("border-radius: {value}"), &mut style)
                .unwrap_or_else(|error| panic!("border-radius: {value} should parse: {error}"));
            style
        };

        let one = apply("4px");
        assert_eq!(one.resolved_corner_radii(), crate::style::CornerRadii::all(4));
        assert_eq!(one.border_radius, Some(4), "the scalar keeps describing a uniform shape");

        let two = apply("4px 8px");
        assert_eq!(
            two.resolved_corner_radii(),
            crate::style::CornerRadii::new(4, 8, 4, 8),
            "two values are (top/bottom, left/right)"
        );

        let three = apply("4px 8px 12px");
        assert_eq!(
            three.resolved_corner_radii(),
            crate::style::CornerRadii::new(4, 8, 12, 8),
            "three values are (top-left, top-right/bottom-left, bottom-right)"
        );

        let four = apply("1px 2px 3px 4px");
        assert_eq!(
            four.resolved_corner_radii(),
            crate::style::CornerRadii::new(1, 2, 3, 4),
            "four values are clockwise from the top-left"
        );
        assert_eq!(
            four.border_radius, None,
            "a non-uniform shape cannot be written into the scalar honestly"
        );
    }

    // ── The two properties the table claimed but the apply did not implement (G-404) ──

    /// `font-weight` reaches `Font`'s own weight channel, numerically or by name.
    #[test]
    fn font_weight_sets_the_fonts_weight() {
        for (value, expected) in
            [("700", 700u16), ("400", 400), ("bold", 700), ("normal", 400), ("450", 500)]
        {
            let mut style = WidgetStyle::default();
            CssParser::apply_declaration_text(&format!("font-weight: {value}"), &mut style)
                .unwrap_or_else(|error| panic!("font-weight: {value} should parse: {error}"));
            let font = style.font.expect("a weight needs a font to live on");
            assert_eq!(font.weight(), expected, "font-weight: {value}");
        }

        let error =
            CssParser::apply_declaration_text("font-weight: heavy", &mut WidgetStyle::default())
                .expect_err("an unsupported weight word must be refused");
        assert!(error.contains("font-weight"), "{error}");
    }

    /// `border-style` reaches the style primitive, and an unsupported line style is refused
    /// rather than painted as the default.
    #[test]
    fn border_style_sets_the_border_style() {
        for (value, expected) in [
            ("solid", crate::style::BorderStyle::Solid),
            ("dashed", crate::style::BorderStyle::Dashed),
            ("Dotted", crate::style::BorderStyle::Dotted),
            ("none", crate::style::BorderStyle::None),
        ] {
            let mut style = WidgetStyle::default();
            CssParser::apply_declaration_text(&format!("border-style: {value}"), &mut style)
                .unwrap_or_else(|error| panic!("border-style: {value} should parse: {error}"));
            assert_eq!(style.border_style, Some(expected), "border-style: {value}");
        }

        let error =
            CssParser::apply_declaration_text("border-style: wavy", &mut WidgetStyle::default())
                .expect_err("an unsupported line style must be refused");
        assert!(error.contains("border-style"), "{error}");
    }

    // ── Cache eviction (G-161) ──────────────────────────────────────────

    /// Overflowing the declaration cache evicts the coldest entry instead of clearing the table.
    #[test]
    fn the_declaration_cache_evicts_one_entry_instead_of_clearing() {
        CssParser::clear_decl_cache();
        // Fill past the limit with distinct lengths; each is its own cache key.
        for value in 0..(DECL_CACHE_LIMIT + 8) {
            let declaration = format!("border-width: {value}px");
            CssParser::apply_declaration_text(&declaration, &mut WidgetStyle::default())
                .unwrap_or_else(|error| panic!("{declaration} should parse: {error}"));
        }
        assert_eq!(
            CssParser::decl_cache_len(),
            DECL_CACHE_LIMIT,
            "the cache keeps its capacity; a whole-table clear would leave only the tail"
        );
        assert!(
            CssParser::decl_cache_contains(&format!("{}px", DECL_CACHE_LIMIT + 7)),
            "the newest value must stay warm"
        );
        assert!(!CssParser::decl_cache_contains("0px"), "the oldest value must be the one evicted");

        // A hit refreshes the stamp, so the touched entry survives the next eviction.
        let touched = format!("{}px", DECL_CACHE_LIMIT + 1);
        CssParser::apply_declaration_text(
            &format!("border-width: {touched}"),
            &mut WidgetStyle::default(),
        )
        .expect("a cache hit");
        CssParser::apply_declaration_text("border-width: 99999px", &mut WidgetStyle::default())
            .expect("one more value, forcing an eviction");
        assert!(
            CssParser::decl_cache_contains(&touched),
            "a recently used entry must survive eviction"
        );
    }

    /// `A B` and `A > B` are different selectors, and neither is a compound AND.
    #[test]
    fn combinators_parse_as_chains() {
        let descendant = CssParser::parse_selector("Panel Button").expect("a descendant chain");
        assert_eq!(
            descendant,
            CssSelector::Descendant(
                Box::new(CssSelector::Kind("Panel".to_string())),
                Box::new(CssSelector::Kind("Button".to_string())),
            )
        );

        let child = CssParser::parse_selector("Panel > Button").expect("a child chain");
        assert_eq!(
            child,
            CssSelector::Child(
                Box::new(CssSelector::Kind("Panel".to_string())),
                Box::new(CssSelector::Kind("Button".to_string())),
            )
        );
        // Whitespace around `>` does not create a second, descendant join.
        assert_eq!(CssParser::parse_selector("Panel>Button"), Some(child));
    }

    /// A compound on either side of a combinator keeps its parts together.
    #[test]
    fn a_compound_is_not_split_by_the_combinator_parser() {
        let parsed = CssParser::parse_selector("Panel.primary > Button:hover").expect("a chain");
        assert_eq!(
            parsed,
            CssSelector::Child(
                Box::new(CssSelector::And(vec![
                    CssSelector::Kind("Panel".to_string()),
                    CssSelector::Class("primary".to_string()),
                ])),
                Box::new(CssSelector::And(vec![
                    CssSelector::Kind("Button".to_string()),
                    CssSelector::State(PseudoState::Hover),
                ])),
            )
        );
    }

    /// A truncated chain is refused rather than silently degraded into its first compound.
    #[test]
    fn a_chain_without_a_subject_is_refused() {
        assert_eq!(CssParser::parse_selector("Panel >"), None);
        assert_eq!(CssParser::parse_selector("> Button"), None);
    }

    /// The canonical `Selector` conversion carries the chain, and specificity sums the
    /// compounds without counting the combinator.
    #[test]
    fn a_parsed_chain_converts_and_carries_its_weight() {
        let parsed = CssParser::parse_selector("Panel > Button").expect("a chain");
        let selector = parsed.to_selector().expect("both kinds are known");
        assert_eq!(selector.specificity(), 2);
        assert!(matches!(selector, Selector::Child { .. }));
    }
}
