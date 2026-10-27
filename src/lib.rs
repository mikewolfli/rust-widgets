// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! rust_widgets - cross-platform native GUI architecture in pure Rust.
//!
//! # Text coverage
//!
//! **The default build draws Latin/ASCII only.** This is a stated boundary, not a bug
//! report: the crate ships no font data, so glyphs come from a fixed 8x8 bitmap face whose
//! character set is `U+0000`–`U+007F`.
//!
//! ```text
//! src/render/text/glyph_source.rs   Font8x8Source  // U+0000-U+007F: the default's only face
//! src/render/text/glyph_source.rs   TOFU           // the fallback glyph for anything else
//! src/render/text/line.rs           estimate_cluster_advance // the one advance model
//! ```
//!
//! What that means in practice:
//!
//! * A character inside the supported range is drawn from the bitmap face.
//! * Any character outside it — CJK, Cyrillic, Arabic, emoji, and every other script —
//!   is drawn as the **fallback glyph** (a hollow box, "tofu"). The label still lays
//!   out and the control still renders; only the glyph is wrong.
//! * A line is ordered for painting by the Unicode bidirectional algorithm, so an Arabic
//!   or Hebrew run is drawn right to left. Ordering is not glyph *shape*: script-specific
//!   shaping (Arabic joining, Indic reordering) is still not applied.
//!
//! ## Coverage beyond Latin/ASCII is opt-in, and that is the whole contract
//!
//! What a wider script needs is font data, and the default build carries **none** — a
//! `mini` or `embedded` profile must not pay hundreds of kilobytes for glyphs it will
//! never draw. So coverage is a feature a caller asks for by name:
//!
//! ```text
//! cargo build --features "desktop,fonts-cjk-bitmap"
//! ```
//!
//! `fonts-cjk-bitmap` adds a generated 16x16 CJK face — Han, kana, CJK punctuation and
//! fullwidth forms, about 85 KB of read-only data read on demand and never resident. Latin
//! rendering stays byte-identical when it is enabled: a face is *appended* to the fallback
//! stack, so it can only answer for characters the base face has no glyph for.
//!
//! ## What is deliberately not claimed
//!
//! This crate does **not** implement Unicode text rendering, and does not call itself
//! multilingual. A host that passes non-Latin text with no font feature enabled will see
//! the fallback glyph, and that is the honest behaviour of the default configuration.
//! Shaping for complex scripts remains future work rather than something assumed.
//!
//! ## The guarantee this section carries
//!
//! The boundary above is asserted from **both** sides — see
//! `render::pipeline::pixel_ops::text_coverage_tests` and `render::text::glyph_source`'s
//! tests. A non-Latin character must resolve to the fallback glyph on a default build, and
//! enabling the CJK data must move that boundary by exactly the face it adds. A claim about
//! what is *not* supported decays silently in either direction, so both are pinned, and
//! `tools/check_text_coverage_claim_matches_features.sh` fails if this section and the
//! feature graph ever disagree.

// BLUE11 R9.6: Unsafe code audit — unsafe is required for platform FFI
// Note: Removed `#![allow(unsafe_code)]` — default is allow, no-op.
// `missing_docs` is denied rather than allowed: every public item in every profile
// is documented, and this lint is what keeps it that way. It is enforced on the
// `desktop`, `embedded`, `mini` and `--all-features` builds alike, and CI's clippy
// job runs with `-D warnings`, so a new undocumented public item fails the build.
#![deny(missing_docs)]
// The OpenHarmony cross target expands `thread_local!` through a std shim whose const path the
// lint cannot see, so every already-`const` initialiser there is reported as "can be made const".
// The lint is accurate on the host targets (where it stays enabled); scoping the allow to the
// OHOS environment keeps that coverage without silencing the check everywhere.
#![cfg_attr(target_env = "ohos", allow(clippy::missing_const_for_thread_local))]
// `mini` is the no-std-oriented profile: it compiles against `core` + `alloc` only.
// Every shared type is imported through `compat`, so this attribute is the switch
// that turns the abstraction into an enforced contract.
#![cfg_attr(feature = "mini", no_std)]
// BLUE11: Clippy lints enabled for quality enforcement.
// Individual allows are placed next to their specific violations.
#![cfg_attr(
    test,
    allow(
        clippy::needless_pass_by_value,
        clippy::unwrap_used,
        clippy::field_reassign_with_default
    )
)]
// Required unconditionally — `alloc` is available in both std (re-exported)
// and no_std contexts. `core` is always available via `extern crate std` under
// std, but we need direct `alloc::` paths in `compat` for no_std builds.
extern crate alloc;

// `#![no_std]` removes the *standard* prelude, and with it the `thread_local!`
// macro. `thread_local!` is not part of the `core` prelude: it is declared in
// `std` and reaches an ordinary crate only through the `#[prelude_import]`
// `std::prelude::rust_20xx` glob that the compiler injects when `std` is
// implicitly linked. Under `mini` the compiler stops injecting that glob (the
// crate is `no_std`), so every `thread_local!` call site in the crate fails to
// resolve — a *macro*-only regression, which is why it breaks modules such as
// `layout` that never touch `std` directly.
//
// The attribute below makes the compiler re-inject the std prelude even though
// `#![no_std]` is in effect, which is exactly what this crate wants: it is
// no-std-*oriented* and still links `std` on every target it builds for, and the
// glob is purely lexical (no `std` item is brought into scope — `Option`,
// `String`, shadows and the rest stay `core`/`compat`-sourced as before).
//
// Two alternatives were measured and rejected:
// * spelling each call site `std::thread_local!` — the macro is not reachable as
//   a path under `no_std` (verified: `cannot find `thread_local` in `std``), so
//   this does not compile at all;
// * `use std::thread_local;` — a macro-only import of a macro that is *not*
//   exported from the `std` crate root under `no_std`, same failure.
#[cfg(alloc_frugal)]
#[macro_use]
extern crate std;

// `#![no_std]` also removes the *alloc* prelude's trait imports, and unlike the
// macros above they cannot be brought back wholesale.
//
// `ToString` is not in the `alloc` prelude at all: it is re-exported by
// `std::prelude::v1` (`library/std/src/prelude/v1.rs`), which a crate receives
// only through the compiler-injected prelude glob that `#![no_std]` suppresses.
// The consequence is that every `s.to_string()` in the crate — on `&str`,
// `char`, `&&str`, `Shortcut`, `io::Error` — stops resolving under `mini`.
//
// This cannot be repaired from here either. Name resolution in Rust is *lexical*:
// a `use` is visible to items lexically nested in the block or module containing
// it, never to a module reached by path. A crate-root `use std::string::ToString;`
// would fix this file and nothing else, and the ninety-odd call sites live in
// child modules. Verified against a `#![no_std]` crate whose root imported the
// trait while a child module called `.to_string()`: the child still fails E0599.
//
// `#[macro_use]` is the mechanism that *does* cross module boundaries, but it
// only carries the macro namespace — and `ToOwned`, the sibling that supplies
// `.to_owned()`, is a trait, not a macro.
//
// So each module that needs these methods imports the trait itself, from
// `compat`, which re-exports it as [`compat::MiniToString`]. The alias keeps the
// import list readable next to `String` and `ToString` on desktop builds, where
// both names resolve.
//
// **BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std ──**
// All crate files import from `compat` instead of directly from std.
pub mod compat;

/// Action/command system.
pub mod action;
/// Generic asset file watcher.
///
/// Gated on the capability it actually needs (`desktop-runtime` supplies
/// `notify` + `crossbeam-channel`), not on the `desktop` profile: `tablet` and
/// `mobile` enable the same capability, so gating on the profile removed a
/// documented public path from builds that can support it (principle #41).
#[cfg(all(feature = "desktop-runtime", not(alloc_frugal)))]
pub mod asset;
/// Audio module — format detection, decoding, encoding, sample processing, and normalization.
#[cfg(feature = "audio")]
pub mod audio;
/// C ABI and language bindings (C / Java-JNI / etc.).
///
/// Available on the `desktop` profile and on any profile that exposes an
/// FFI consumer (`jni` for Java, `mobile-api` for the mobile runtime). The
/// `mini` profile excludes it because it is `alloc`-free / no-std oriented.
#[cfg(all(any(feature = "desktop", feature = "jni", feature = "mobile-api"), not(alloc_frugal)))]
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Reactive data binding system — Model → View automatic synchronization.
pub mod data_binding;
/// Designer support: mode 2, generating Rust source from a project (BLUE19 T-23, D7-b-3).
///
/// # Why this has its own gate rather than riding on `full_widgets`
///
/// `full_widgets` answers "does this build have the widget tree and the capability table?", which
/// every device application needs. This module answers a different question — "is this build **also**
/// a design tool?" — and a shipping application does not.
///
/// The generator and its artifact writer are development-time facilities. Leaving them on for every
/// profile would link a code generator plus `std::fs::write` into delivery artifacts, which is the
/// weight mode 2 exists to remove (BLUE19 §5.1.2).
///
/// `desktop` enables the `designer` feature by default — it is the profile a designer **host** runs
/// on. `tablet`/`mobile`/`mini`/`embedded` do not, because they are the **targets** of a generation,
/// not its host (BLUE19 §5.3.4); a caller who wants the generator there asks for it:
/// `--features tablet,designer`.
///
/// The condition is expressed as the `designer_tooling` alias from `build.rs` rather than a
/// hand-written conjunction, per rule #47.
#[cfg(designer_tooling)]
pub mod designer;
/// Embedded system optimizations and support.
/// Unified error system (ErrorId, RwError, c_try!).
pub mod error;
/// Event types and dispatch helpers.
pub mod event;
/// Gesture recognizer system (gated behind `touch` feature).
#[cfg(feature = "touch")]
pub mod gesture;
/// Hardware-adaptive GPU management.
pub mod gpu;
/// Internationalization module.
#[cfg(feature = "i18n")]
pub mod i18n;
/// Image module — format detection, decoding, encoding, transform, and color conversion.
///
/// **Decoding** is real for: PNG (all bit depths, scanline filters, palette), JPEG,
/// BMP, QOI, Farbfeld, PNM (P5/P6). GIF/WebP/TIFF/AVIF/ICO/SVG decode returns an
/// explicit `Err` rather than fabricated pixels until a codec lands.
#[cfg(feature = "image")]
pub mod image;
/// Declarative JSON window engine (declarative-tree style).
///
/// Compiled for a full device build — `desktop`/`tablet`/`mobile` with an unstripped
/// widget set. Stated as the `full_widgets` alias rather than the conjunction, so it
/// cannot drift from `crate::app` and `crate::view`, which need exactly the same thing
/// (BLUE15 rule #57).
#[cfg(full_widgets)]
pub mod json;
/// Layout managers.
pub mod layout;
/// Memory management utilities.
pub mod memory;
/// Advanced widgets (gated behind `advanced-widgets` feature).
#[cfg(feature = "advanced-widgets")]
pub mod menu_config;
/// Object tree and object utilities.
pub mod object;
/// Performance monitoring and optimization.
pub mod performance;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Quality management for adaptive rendering.
pub mod quality;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Persistent application settings (`settings store`-equivalent key/value store).
///
/// Gated like the existing menu-config persistence precedent (`advanced-widgets`) plus
/// `serde_json`, and absent from `alloc_frugal` builds where `std::fs` is unavailable.
#[cfg(all(feature = "advanced-widgets", feature = "serde_json", not(alloc_frugal)))]
pub mod settings;
#[cfg(all(feature = "advanced-widgets", feature = "serde_json", not(alloc_frugal)))]
pub use settings::{
    JsonSettings, MemorySettings, SettingValue, Settings, SettingsError, SettingsStore,
};
/// Global shortcut system for keyboard shortcuts.
pub mod shortcut;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
/// Test infrastructure and utilities.
pub mod test;
/// Theme management — the palette, the appearance selector and the role table controls
/// resolve against, plus the process-wide manager that selects among them.
///
/// # Gating (BLUE20 layer 4, 2026-09-21)
///
/// Gated on `device_profile` (`desktop`/`tablet`/`mobile`), which is a *conjunction*: the
/// theme model needs a colour system (`not(alloc_frugal)`) **and** `serde`, and only the
/// three device profiles enable both. `mini` has no palette; the mixed
/// `windows + controls-custom` build has no `serde`.
///
/// # Why the widget layer no longer breaks when this is absent
///
/// ~120 control files read the theme while drawing, and they also compile in
/// `--no-default-features --features "windows desktop-runtime controls-custom"`
/// — a full widget set with no device profile. (`controls-native` is a documented
/// no-op alias and is deliberately omitted, so the example names only what it
/// actually selects.) Referencing
/// `crate::theme` from those files was 78 compile errors at HEAD. They now go through
/// `crate::style`, which exposes the same lookups (and the same type *shape* where the real
/// types are absent) and answers "no theme" there, so one path compiles in every profile
/// and the widgets fall back to their own literals where there is no palette to read
/// (principles #37/#47: a missing capability is reported, and the condition lives in one
/// place rather than at each call site).
#[cfg(device_profile)]
pub mod theme;
/// Undo/Redo framework for undoable commands and cross-widget undo/redo.
pub mod undo;
/// Video module — container format detection, frame extraction, metadata, and playback.
#[cfg(feature = "video")]
pub mod video;
/// Generic utility modules (asset watcher, helpers, etc.).
/// Declarative-retained view layer: `state → Node → diff → Patch → retained tree`.
///
/// The declarative half of the hybrid architecture. Additive: it consumes the existing
/// widget factory and property contract rather than changing them.
///
/// Compiled for a device profile (`desktop`/`tablet`/`mobile`) unless the caller opts
/// out with the `no-declarative-view` feature; never compiled for `mini`/`embedded`.
/// The whole condition is the single alias `declarative_view`, so the opt-out and the
/// stripped-profile rule cannot drift apart (BLUE15 rule #57).
#[cfg(declarative_view)]
pub mod view;
/// Web view and engine components.
#[cfg(widgets_unstripped)]
pub mod web;
/// Optional WGPU GPU acceleration backend (gated behind `gpu-wgpu` feature).
#[cfg(feature = "gpu-wgpu")]
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
// Re-export the widget surface explicitly.
//
// Generated from `widget/mod.rs` so the two lists start out identical; the point
// is that this list is now the audit surface (BIND-13): a new control must be
// added here (or it is deliberately not part of the crate root), instead of a
// wildcard silently exporting whatever the module happens to declare.
#[cfg(not(alloc_frugal))]
pub use widget::a11y_submit;
#[cfg(full_widgets)]
pub use widget::advanced_widgets;
pub use widget::base;
pub use widget::base_property_get;
pub use widget::base_property_set;
pub use widget::base_widgets;
pub use widget::capability;
#[cfg(full_widgets)]
pub use widget::census;
#[cfg(full_widgets)]
pub use widget::chart_widgets;
#[cfg(widgets_unstripped)]
pub use widget::clear_registered_icons;
#[cfg(full_widgets)]
pub use widget::composite;
pub use widget::container_widgets;
#[cfg(full_widgets)]
pub use widget::cupertino;
pub use widget::decorations;
#[cfg(full_widgets)]
pub use widget::dialog;
pub use widget::display_widgets;
pub use widget::draw;
pub use widget::draw_bridge;
pub use widget::draw_icon_at;
pub use widget::draw_icon_centered;
#[cfg(feature = "image")]
pub use widget::draw_icon_image;
pub use widget::draw_icon_named;
pub use widget::fit_image_into;
#[cfg(widgets_unstripped)]
pub use widget::icon_registry;
pub use widget::icon_source;
pub use widget::icon_token_is_drawable;
pub use widget::input_widgets;
#[cfg(widgets_unstripped)]
pub use widget::is_registered_icon;
pub use widget::kind;
pub use widget::measure_icon;
#[cfg(full_widgets)]
pub use widget::media_widgets;
#[cfg(full_widgets)]
pub use widget::menu_toolbar;
pub use widget::metrics;
#[cfg(full_widgets)]
pub use widget::misc_widgets;
#[cfg(full_widgets)]
pub use widget::nav_widgets;
pub use widget::numeric;
#[cfg(full_widgets)]
pub use widget::overlay_widgets;
pub use widget::paint_icon;
#[cfg(widgets_unstripped)]
pub use widget::read_widget_property_by_id;
#[cfg(widgets_unstripped)]
pub use widget::register_icon;
#[cfg(widgets_unstripped)]
pub use widget::register_icon_on_grid;
#[cfg(widgets_unstripped)]
pub use widget::registered_icon;
#[cfg(widgets_unstripped)]
pub use widget::registered_icon_count;
pub use widget::registry;
#[cfg(not(alloc_frugal))]
pub use widget::runtime;
pub use widget::sample_data;
#[cfg(full_widgets)]
pub use widget::sample_fill;
#[cfg(full_widgets)]
pub use widget::special_widgets;
pub use widget::svg;
pub use widget::text_utils;
#[cfg(widgets_unstripped)]
pub use widget::try_register_icon;
#[cfg(widgets_unstripped)]
pub use widget::try_register_icon_on_grid;
pub use widget::validate_icon;
#[cfg(full_widgets)]
pub use widget::view_widgets;
#[cfg(full_widgets)]
pub use widget::web_widgets;
pub use widget::widget_property_get;
pub use widget::widget_property_names;
pub use widget::widget_property_set;
pub use widget::widget_trait;
pub use widget::window;
#[cfg(widgets_unstripped)]
pub use widget::write_widget_property_by_id;
#[cfg(full_widgets)]
pub use widget::Action;
#[cfg(full_widgets)]
pub use widget::ActivityIndicator;
#[cfg(full_widgets)]
pub use widget::AdaptiveScaffold;
#[cfg(full_widgets)]
pub use widget::AnimatedFrame;
#[cfg(full_widgets)]
pub use widget::AnimatedImage;
#[cfg(full_widgets)]
pub use widget::AnimatedImageFormat;
#[cfg(full_widgets)]
pub use widget::AppBar;
pub use widget::Arc;
#[cfg(full_widgets)]
pub use widget::AudioVisualizer;
#[cfg(widgets_unstripped)]
pub use widget::AutoCompleteEdit;
#[cfg(full_widgets)]
pub use widget::Avatar;
#[cfg(widgets_unstripped)]
pub use widget::Badge;
#[cfg(full_widgets)]
pub use widget::Banner;
#[cfg(full_widgets)]
pub use widget::BarChart;
#[cfg(full_widgets)]
pub use widget::BarEntry;
#[cfg(full_widgets)]
pub use widget::BarcodeFormat;
#[cfg(full_widgets)]
pub use widget::BarcodeResult;
#[cfg(full_widgets)]
pub use widget::BarcodeScanner;
pub use widget::BaseWidget;
#[cfg(full_widgets)]
pub use widget::BezierCurveEditor;
#[cfg(full_widgets)]
pub use widget::BottomNavigationBar;
#[cfg(full_widgets)]
pub use widget::BottomSheet;
#[cfg(full_widgets)]
pub use widget::Breadcrumb;
#[cfg(full_widgets)]
pub use widget::BreadcrumbSegment;
pub use widget::Button;
pub use widget::ButtonState;
#[cfg(full_widgets)]
pub use widget::Calendar;
#[cfg(full_widgets)]
pub use widget::CameraPreview;
#[cfg(full_widgets)]
pub use widget::Canvas;
pub use widget::CapabilityAccessError;
pub use widget::CapabilityValue;
#[cfg(widgets_unstripped)]
pub use widget::Carousel;
#[cfg(widgets_unstripped)]
pub use widget::CarouselIndicatorPosition;
#[cfg(widgets_unstripped)]
pub use widget::CarouselIndicatorStyle;
#[cfg(widgets_unstripped)]
pub use widget::CarouselPage;
#[cfg(full_widgets)]
pub use widget::Cascader;
#[cfg(full_widgets)]
pub use widget::CascaderOption;
#[cfg(full_widgets)]
pub use widget::Chart;
#[cfg(full_widgets)]
pub use widget::ChartWidget;
pub use widget::CheckBox;
#[cfg(full_widgets)]
pub use widget::CheckListBox;
pub use widget::CheckState;
#[cfg(full_widgets)]
pub use widget::Chip;
#[cfg(full_widgets)]
pub use widget::ChipItem;
#[cfg(full_widgets)]
pub use widget::CodeEditor;
#[cfg(widgets_unstripped)]
pub use widget::CollapsiblePane;
#[cfg(full_widgets)]
pub use widget::ColorDialog;
#[cfg(widgets_unstripped)]
pub use widget::ColorHistory;
#[cfg(full_widgets)]
pub use widget::ColorPicker;
#[cfg(widgets_unstripped)]
pub use widget::ColorWell;
#[cfg(full_widgets)]
pub use widget::ColumnFilter;
#[cfg(full_widgets)]
pub use widget::ColumnView;
pub use widget::ComboBox;
#[cfg(full_widgets)]
pub use widget::CommandEntry;
#[cfg(widgets_unstripped)]
pub use widget::CommandLink;
#[cfg(full_widgets)]
pub use widget::CommandPalette;
#[cfg(full_widgets)]
pub use widget::Compass;
#[cfg(full_widgets)]
pub use widget::CompletedMention;
#[cfg(full_widgets)]
pub use widget::ContextMenu;
#[cfg(full_widgets)]
pub use widget::CupertinoAlertDialog;
#[cfg(full_widgets)]
pub use widget::CupertinoDatePicker;
#[cfg(full_widgets)]
pub use widget::CupertinoNavigationBar;
#[cfg(full_widgets)]
pub use widget::CupertinoSegmentedControl;
#[cfg(full_widgets)]
pub use widget::CupertinoSlider;
#[cfg(full_widgets)]
pub use widget::CupertinoSwitch;
#[cfg(full_widgets)]
pub use widget::DataGrid;
#[cfg(full_widgets)]
pub use widget::DataView;
#[cfg(full_widgets)]
pub use widget::DateEdit;
#[cfg(full_widgets)]
pub use widget::DateFormat;
#[cfg(full_widgets)]
pub use widget::DatePicker;
#[cfg(full_widgets)]
pub use widget::DateRangePicker;
#[cfg(full_widgets)]
pub use widget::DateTimeEdit;
#[cfg(full_widgets)]
pub use widget::DateTimePicker;
#[cfg(full_widgets)]
pub use widget::Dependency;
#[cfg(full_widgets)]
pub use widget::DependencyEditor;
#[cfg(full_widgets)]
pub use widget::DiagnosticMarker;
#[cfg(full_widgets)]
pub use widget::Dial;
#[cfg(full_widgets)]
pub use widget::Dialog;
#[cfg(full_widgets)]
pub use widget::DiffFileList;
#[cfg(full_widgets)]
pub use widget::DiffKind;
#[cfg(full_widgets)]
pub use widget::DiffLine;
#[cfg(full_widgets)]
pub use widget::DiffViewer;
#[cfg(full_widgets)]
pub use widget::DirectoryDialog;
#[cfg(widgets_unstripped)]
pub use widget::Divider;
#[cfg(widgets_unstripped)]
pub use widget::DockPanel;
#[cfg(widgets_unstripped)]
pub use widget::DockWidget;
#[cfg(full_widgets)]
pub use widget::DoubleSpinBox;
pub use widget::Draw;
#[cfg(full_widgets)]
pub use widget::DropZone;
pub use widget::Dropdown;
#[cfg(full_widgets)]
pub use widget::DropdownItem;
#[cfg(full_widgets)]
pub use widget::DropdownMenu;
pub use widget::EchoMode;
#[cfg(widgets_unstripped)]
pub use widget::EditableComboBox;
#[cfg(full_widgets)]
pub use widget::EmojiGlyph;
#[cfg(full_widgets)]
pub use widget::EmojiPicker;
#[cfg(widgets_unstripped)]
pub use widget::EmptyState;
#[cfg(full_widgets)]
pub use widget::FileDialog;
#[cfg(full_widgets)]
pub use widget::FilterCondition;
#[cfg(full_widgets)]
pub use widget::FilterConjunction;
#[cfg(full_widgets)]
pub use widget::FilterExpr;
#[cfg(full_widgets)]
pub use widget::FilterField;
#[cfg(full_widgets)]
pub use widget::FilterOperator;
#[cfg(full_widgets)]
pub use widget::FindBar;
#[cfg(full_widgets)]
pub use widget::FindReplaceDialog;
#[cfg(widgets_unstripped)]
pub use widget::FloatingLabel;
#[cfg(widgets_unstripped)]
pub use widget::FontComboBox;
#[cfg(full_widgets)]
pub use widget::FontDialog;
#[cfg(widgets_unstripped)]
pub use widget::FontPreview;
pub use widget::Frame;
#[cfg(full_widgets)]
pub use widget::FreeformShape;
#[cfg(full_widgets)]
pub use widget::FreeformShapeWidget;
#[cfg(full_widgets)]
pub use widget::GalleryImage;
#[cfg(full_widgets)]
pub use widget::GanttTask;
#[cfg(full_widgets)]
pub use widget::GanttWidget;
#[cfg(full_widgets)]
pub use widget::Grid;
#[cfg(full_widgets)]
pub use widget::GridTable;
#[cfg(full_widgets)]
pub use widget::GridTableWidget;
#[cfg(full_widgets)]
pub use widget::GridViewHeader;
#[cfg(full_widgets)]
pub use widget::GridWidget;
pub use widget::GroupBox;
#[cfg(full_widgets)]
pub use widget::Heatmap;
#[cfg(full_widgets)]
pub use widget::HeatmapCell;
#[cfg(full_widgets)]
pub use widget::HeroAnimation;
#[cfg(widgets_unstripped)]
pub use widget::Icon;
pub use widget::IconColorMode;
pub use widget::IconData;
pub use widget::IconItemError;
pub use widget::IconMetrics;
pub use widget::IconName;
pub use widget::IconPaintError;
pub use widget::IconPaintOutcome;
pub use widget::IconRef;
#[cfg(widgets_unstripped)]
pub use widget::IconRegistrationError;
pub use widget::IconResourceResolver;
pub use widget::IconSource;
pub use widget::IconSourceMetadata;
pub use widget::IconSpec;
#[cfg(feature = "image")]
pub use widget::Image;
#[cfg(feature = "image")]
pub use widget::ImageFormat;
#[cfg(full_widgets)]
pub use widget::ImageGallery;
#[cfg(feature = "image")]
pub use widget::ImageView;
#[cfg(widgets_unstripped)]
pub use widget::ImePreedit;
#[cfg(widgets_unstripped)]
pub use widget::InplaceEditor;
#[cfg(full_widgets)]
pub use widget::InputDialog;
#[cfg(full_widgets)]
pub use widget::KanbanBoard;
#[cfg(full_widgets)]
pub use widget::KanbanCard;
#[cfg(full_widgets)]
pub use widget::KanbanCardView;
#[cfg(full_widgets)]
pub use widget::KanbanColumn;
#[cfg(full_widgets)]
pub use widget::KanbanColumnHeader;
#[cfg(full_widgets)]
pub use widget::KeySequenceEdit;
pub use widget::Keyboard;
#[cfg(widgets_unstripped)]
pub use widget::LCDNumber;
pub use widget::Label;
pub use widget::Line;
#[cfg(full_widgets)]
pub use widget::LineChart;
pub use widget::LineEdit;
pub use widget::LineOrientation;
pub use widget::ListBox;
#[cfg(full_widgets)]
pub use widget::ListModel;
#[cfg(full_widgets)]
pub use widget::ListView;
#[cfg(full_widgets)]
pub use widget::LottieWidget;
#[cfg(full_widgets)]
pub use widget::MapMarker;
#[cfg(full_widgets)]
pub use widget::MapView;
#[cfg(full_widgets)]
pub use widget::MarkdownEditor;
#[cfg(full_widgets)]
pub use widget::MarkdownOutlinePanel;
#[cfg(full_widgets)]
pub use widget::MarkerSeverity;
#[cfg(widgets_unstripped)]
pub use widget::MaskedEdit;
#[cfg(widgets_unstripped)]
pub use widget::MasonryItem;
#[cfg(widgets_unstripped)]
pub use widget::MasonryLayout;
#[cfg(full_widgets)]
pub use widget::MaterialNavigationRail;
#[cfg(full_widgets)]
pub use widget::MaterialSnackbar;
#[cfg(widgets_unstripped)]
pub use widget::MdiArea;
#[cfg(full_widgets)]
pub use widget::MediaPlayer;
#[cfg(full_widgets)]
pub use widget::Mention;
#[cfg(full_widgets)]
pub use widget::MentionCandidate;
#[cfg(full_widgets)]
pub use widget::Menu;
#[cfg(full_widgets)]
pub use widget::MenuBar;
#[cfg(full_widgets)]
pub use widget::MenuButton;
#[cfg(full_widgets)]
pub use widget::MenuItem;
#[cfg(full_widgets)]
pub use widget::MessageBox;
pub use widget::Meter;
pub use widget::MiniCanvas;
pub use widget::MiniChart;
#[cfg(full_widgets)]
pub use widget::MobileDatePicker;
#[cfg(full_widgets)]
pub use widget::ModalBottomSheet;
#[cfg(widgets_unstripped)]
pub use widget::MultiSelectComboBox;
#[cfg(widgets_unstripped)]
pub use widget::MultiSelectItem;
#[cfg(full_widgets)]
pub use widget::NavItem;
#[cfg(full_widgets)]
pub use widget::NavigationDrawer;
#[cfg(full_widgets)]
pub use widget::NavigationEvent;
#[cfg(full_widgets)]
pub use widget::NavigationStack;
#[cfg(full_widgets)]
pub use widget::NotificationCenter;
#[cfg(full_widgets)]
pub use widget::NotificationItem;
#[cfg(full_widgets)]
pub use widget::NotificationLevel;
#[cfg(widgets_unstripped)]
pub use widget::NumberPicker;
#[cfg(widgets_unstripped)]
pub use widget::OtpInput;
#[cfg(full_widgets)]
pub use widget::Pagination;
pub use widget::Panel;
#[cfg(full_widgets)]
pub use widget::PieChart;
#[cfg(full_widgets)]
pub use widget::PieMenu;
#[cfg(full_widgets)]
pub use widget::PieMenuItem;
#[cfg(full_widgets)]
pub use widget::PieSlice;
#[cfg(full_widgets)]
pub use widget::Popover;
#[cfg(full_widgets)]
pub use widget::PopupWindow;
pub use widget::ProgressBar;
#[cfg(widgets_unstripped)]
pub use widget::ProgressCircle;
#[cfg(full_widgets)]
pub use widget::ProgressDialog;
#[cfg(full_widgets)]
pub use widget::PropertiesPanel;
#[cfg(full_widgets)]
pub use widget::PropertyEntry;
#[cfg(full_widgets)]
pub use widget::PropertyGrid;
#[cfg(full_widgets)]
pub use widget::PropertyItem;
pub use widget::PropertySchema;
#[cfg(full_widgets)]
pub use widget::PropertyValue;
pub use widget::PropertyValueKind;
#[cfg(full_widgets)]
pub use widget::PullToRefresh;
#[cfg(full_widgets)]
pub use widget::QRCode;
#[cfg(full_widgets)]
pub use widget::QueryBuilder;
#[cfg(full_widgets)]
pub use widget::QueryBuilderRow;
#[cfg(full_widgets)]
pub use widget::RadarChart;
pub use widget::RadioButton;
#[cfg(full_widgets)]
pub use widget::RailItem;
#[cfg(full_widgets)]
pub use widget::RangePresetBar;
#[cfg(widgets_unstripped)]
pub use widget::RangeSlider;
#[cfg(widgets_unstripped)]
pub use widget::RangeSliderOrientation;
#[cfg(widgets_unstripped)]
pub use widget::Rating;
#[cfg(full_widgets)]
pub use widget::RefreshControl;
#[cfg(widgets_unstripped)]
pub use widget::RegisteredIcon;
#[cfg(full_widgets)]
pub use widget::RibbonBar;
#[cfg(full_widgets)]
pub use widget::RibbonGroup;
#[cfg(full_widgets)]
pub use widget::RibbonItem;
#[cfg(widgets_unstripped)]
pub use widget::RichEdit;
#[cfg(full_widgets)]
pub use widget::RiveInput;
#[cfg(full_widgets)]
pub use widget::RiveInputValue;
#[cfg(full_widgets)]
pub use widget::RiveWidget;
pub use widget::Roller;
#[cfg(widgets_unstripped)]
pub use widget::SafeArea;
#[cfg(widgets_unstripped)]
pub use widget::SafeAreaInsets;
#[cfg(full_widgets)]
pub use widget::ScaleBar;
#[cfg(full_widgets)]
pub use widget::ScaleBarStyle;
#[cfg(full_widgets)]
pub use widget::ScaleUnits;
pub use widget::ScrollArea;
pub use widget::ScrollBar;
#[cfg(widgets_unstripped)]
pub use widget::SearchBar;
#[cfg(widgets_unstripped)]
pub use widget::SearchBox;
#[cfg(full_widgets)]
pub use widget::Segment;
#[cfg(full_widgets)]
pub use widget::SegmentItem;
#[cfg(full_widgets)]
pub use widget::SegmentedButton;
#[cfg(full_widgets)]
pub use widget::SegmentedControl;
pub use widget::SelectionMode;
#[cfg(full_widgets)]
pub use widget::ShapeCanvas;
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub use widget::SharedIconPixels;
#[cfg(widgets_unstripped)]
pub use widget::ShortcutEditor;
#[cfg(widgets_unstripped)]
pub use widget::ShortcutEntry;
#[cfg(full_widgets)]
pub use widget::SignaturePad;
#[cfg(full_widgets)]
pub use widget::SignatureStroke;
pub use widget::SimpleRegistry;
#[cfg(widgets_unstripped)]
pub use widget::SkeletonLoader;
pub use widget::Slider;
#[cfg(full_widgets)]
pub use widget::Snackbar;
#[cfg(full_widgets)]
pub use widget::SortSpec;
#[cfg(full_widgets)]
pub use widget::Sparkline;
pub use widget::SpinBox;
pub use widget::Spinner;
#[cfg(full_widgets)]
pub use widget::SplashScreen;
#[cfg(full_widgets)]
pub use widget::SplitAction;
#[cfg(full_widgets)]
pub use widget::SplitButton;
#[cfg(widgets_unstripped)]
pub use widget::Splitter;
#[cfg(widgets_unstripped)]
pub use widget::StackedWidget;
#[cfg(full_widgets)]
pub use widget::StatusBar;
#[cfg(widgets_unstripped)]
pub use widget::Stepper;
pub use widget::StickyRegion;
#[cfg(full_widgets)]
pub use widget::SwipeToDismiss;
pub use widget::Switch;
#[cfg(full_widgets)]
pub use widget::TabBar;
#[cfg(full_widgets)]
pub use widget::TabBarTab;
#[cfg(full_widgets)]
pub use widget::TabPage;
#[cfg(full_widgets)]
pub use widget::TabView;
#[cfg(widgets_unstripped)]
pub use widget::TabWidget;
#[cfg(full_widgets)]
pub use widget::Table;
#[cfg(full_widgets)]
pub use widget::TableModel;
#[cfg(full_widgets)]
pub use widget::TableWidget;
#[cfg(widgets_unstripped)]
pub use widget::TagInput;
#[cfg(full_widgets)]
pub use widget::TerminalView;
pub use widget::TextArea;
#[cfg(widgets_unstripped)]
pub use widget::TextEdit;
#[cfg(full_widgets)]
pub use widget::TileKey;
#[cfg(full_widgets)]
pub use widget::TileLoadError;
#[cfg(full_widgets)]
pub use widget::TileLoader;
#[cfg(full_widgets)]
pub use widget::TileRequest;
#[cfg(full_widgets)]
pub use widget::TileViewport;
#[cfg(full_widgets)]
pub use widget::TimeEdit;
#[cfg(full_widgets)]
pub use widget::TimePicker;
#[cfg(full_widgets)]
pub use widget::TimelineAxis;
#[cfg(full_widgets)]
pub use widget::TimelineItem;
#[cfg(full_widgets)]
pub use widget::TimelineWidget;
#[cfg(full_widgets)]
pub use widget::Toast;
#[cfg(full_widgets)]
pub use widget::ToastItem;
#[cfg(full_widgets)]
pub use widget::ToastLevel;
#[cfg(full_widgets)]
pub use widget::ToastStack;
#[cfg(widgets_unstripped)]
pub use widget::ToggleButton;
#[cfg(widgets_unstripped)]
pub use widget::ToggleButtonState;
#[cfg(full_widgets)]
pub use widget::ToolBar;
#[cfg(widgets_unstripped)]
pub use widget::ToolBox;
#[cfg(full_widgets)]
pub use widget::ToolButton;
#[cfg(widgets_unstripped)]
pub use widget::Toolbox;
#[cfg(full_widgets)]
pub use widget::Tooltip;
#[cfg(full_widgets)]
pub use widget::TreeModel;
#[cfg(full_widgets)]
pub use widget::TreeTable;
#[cfg(full_widgets)]
pub use widget::TreeTableModel;
#[cfg(full_widgets)]
pub use widget::TreeView;
#[cfg(full_widgets)]
pub use widget::UndoView;
#[cfg(full_widgets)]
pub use widget::VecListModel;
#[cfg(full_widgets)]
pub use widget::VideoPlayer;
#[cfg(full_widgets)]
pub use widget::VirtualList;
#[cfg(full_widgets)]
pub use widget::VirtualTable;
#[cfg(full_widgets)]
pub use widget::WebEngine;
#[cfg(full_widgets)]
pub use widget::WebEngineContextMenuRequest;
#[cfg(full_widgets)]
pub use widget::WebEngineCookieStore;
#[cfg(full_widgets)]
pub use widget::WebEngineDownloadItem;
#[cfg(full_widgets)]
pub use widget::WebEngineFindTextResult;
#[cfg(full_widgets)]
pub use widget::WebEngineNotification;
#[cfg(full_widgets)]
pub use widget::WebEnginePage;
#[cfg(full_widgets)]
pub use widget::WebEngineScriptDialog;
#[cfg(full_widgets)]
pub use widget::WebEngineSettings;
#[cfg(full_widgets)]
pub use widget::WebEngineView;
#[cfg(full_widgets)]
pub use widget::WebEngineWebChannel;
#[cfg(full_widgets)]
pub use widget::WebFindBar;
#[cfg(full_widgets)]
pub use widget::WebView;
pub use widget::Widget;
#[cfg(widgets_unstripped)]
pub use widget::WidgetAndDraw;
pub use widget::WidgetCapability;
#[cfg(widgets_unstripped)]
pub use widget::WidgetFactory;
pub use widget::WidgetKind;
pub use widget::WidgetProperties;
pub use widget::Window;
#[cfg(full_widgets)]
pub use widget::Wizard;
#[cfg(full_widgets)]
pub use widget::WizardDialog;
#[cfg(full_widgets)]
pub use widget::WizardStep;
#[cfg(full_widgets)]
pub use widget::ZoomButton;
#[cfg(full_widgets)]
pub use widget::ZoomControl;
pub use widget::BASE_PROPERTY_NAMES;
#[cfg(full_widgets)]
pub use widget::FAB;
#[cfg(widgets_unstripped)]
pub use widget::MAX_REGISTERED_ICONS;
#[cfg(not(alloc_frugal))]
pub use widget::{collect_save_state, restore_save_state};
pub use widget::{SaveState, SavedState};

/// Drives every library-side task of one frame, in the order they must happen.
///
/// # Why this is re-exported here rather than reached through `widget::runtime`
///
/// It is the counterpart of [`drain_triggers`]: both are "the library owns the logic,
/// the platform owns the clock" entry points that a backend's event loop calls every
/// frame. `drain_triggers` sits at the crate root because backends call it from
/// `mini` too, where `widget::runtime` is not compiled -- so the frame entry point
/// lives beside it, and the two are read together by every platform loop.
///
/// See [`widget::runtime::drive_frame`] for the ordering contract and the reason it is
/// the crate's single frame driver.
#[cfg(not(alloc_frugal))]
pub use widget::runtime::{
    animation_suspended, drive_frame, last_frame_stats, notify_native_redraw, report_state,
    request_repaint_because, set_animation_suspended, FrameOutcome, FrameStats, RepaintReason,
    StateFact,
};

/// Stops or restarts the animation clock in the allocation-frugal profile.
///
/// `mini`/`embedded` hold no animation bus (see the [`FrameOutcome`] note below), so there is
/// nothing to stop; accepting the call keeps the mobile lifecycle code free of a profile `cfg`
/// at every call site (principle #53).
#[cfg(alloc_frugal)]
pub fn set_animation_suspended(_suspended: bool) {}

/// Whether the animation clock is stopped in the allocation-frugal profile; always `false`,
/// because there is no clock to stop.
#[cfg(alloc_frugal)]
pub fn animation_suspended() -> bool {
    false
}

/// What one frame did, in the profile that has no animation bus.
///
/// # Why the allocation-frugal profile needs a *type* and not just a function
///
/// `mini` has no `widget::runtime` (`Rc`/`RefCell` widget state is what that profile exists
/// to avoid), so it has no animation bus and no driver to speak of. But the platform loops
/// are **shared** -- `android`/`ios`/`harmony`/`wasm` compile into `mini` as well as into the
/// desktop profiles -- and each of them calls `crate::drive_frame` once per iteration.
///
/// The alternative would be a `cfg` at every one of those call sites (or a second, differently
/// named function for one profile), which is the API fork principle #53 forbids: a capability
/// difference is a runtime fact, not two function signatures. So the frame entry point exists
/// in every profile, and in this one it reports the truth -- nothing was ticked, and no frame
/// is owed -- rather than pretending to have driven a bus that is not there.
#[cfg(alloc_frugal)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Events drained from the trigger queue; always `0` in this profile, which has no queue.
    pub events_dispatched: usize,
    /// Controls advanced; always `0`, because this profile holds no widget objects.
    pub controls_ticked: usize,
    /// Repaint requests submitted; always `0`, for the same reason.
    pub repaints_submitted: usize,
    /// Whether another frame is owed; always `false` -- `mini` repaints whole frames on demand
    /// rather than running an animation loop, so it never owes one.
    pub needs_another_frame: bool,
}

/// Drives one frame in the allocation-frugal profile.
///
/// # Why this is a real function and not a stub
///
/// The plan's wording for this profile is "`drive_frame` is a no-op on `mini`/`embedded`, and no
/// stub is provided" (BLUE24 §1.3). The half of that which matters is the *behaviour*: this
/// profile must not run an animation bus, because it has no clock and no widget state to advance.
/// It still needs an entry point, because the platform loops it shares with the desktop profiles
/// call one.
///
/// So this is not a placeholder: it answers honestly for every field. There is nothing to
/// interpolate toward -- a control in this profile is painted whole, on demand, from its program
/// state -- so `needs_another_frame` is `false` and a loop built on it sleeps until the next event.
///
/// # Why it does not drain the trigger queue here
///
/// It deliberately does **not** call [`drain_triggers`], even though that function compiles in this
/// profile (it returns `0`, because there is no trigger queue). Two reasons, and the first is the
/// one that matters:
///
/// 1. `drain_triggers` has exactly **one** owner -- this crate's frame driver. The gate
///    `check_single_frame_driver` asserts that structurally, and a second caller here would make
///    the assertion false: the order the plan fixes (input → advance → report) would be split
///    across two places, and a desktop build that later copied this arm would silently introduce
///    the "two drains per frame" defect the gate exists to catch.
/// 2. In this profile the drain is a no-op anyway, so calling it would buy nothing and would make
///    the code *look* like it did something. An honest `0` says the same thing without pretending.
#[cfg(alloc_frugal)]
pub fn drive_frame(_delta_ms: u32) -> FrameOutcome {
    FrameOutcome::default()
}

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// `tr!` needs a **literal** key, so it cannot serve a caller that holds a key at
/// runtime (a control's `set_translated_tooltip`, a data-driven label). Without this
/// function such a caller has to write its own `cfg` pair, which is exactly how
/// `set_translated_tooltip` ended up gating i18n on `desktop` and silently losing
/// translations on `tablet`/`mobile` (principle #41).
///
/// Having one feature-independent entry point means a call site never needs to know
/// whether the catalogue exists.
///
/// # The no-`i18n` behaviour
///
/// Returns the key itself and warns, matching [`tr!`]: a build without the catalogue
/// must not look localized, and an empty string would be worse than the raw key(which
/// at least names what is missing).
#[cfg(feature = "i18n")]
pub fn translate_key(key: &str) -> crate::compat::String {
    crate::i18n::translate(key)
}

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// See the `i18n` variant's documentation for why this function exists alongside the
/// [`tr!`] macro.
#[cfg(not(feature = "i18n"))]
pub fn translate_key(key: &str) -> crate::compat::String {
    log::warn!("i18n translate_key called but the i18n feature is disabled, key={key}");
    crate::compat::String::from(key)
}
// NOTE: there is no top-level `chart` module. The chart *engine* (layout, axes,
// ticks, SVG context, adapter) and the chart *widgets* live together under
// `crate::widget::chart_widgets`, because they are two layers of one feature.
// The engine is reachable as `rust_widgets::widget::chart_widgets::charts`
// (and `::types`/`::layout`/`::svg`/`::adapter`).
/// Translates a message key — the no-`i18n` fallback spelling.
///
/// This macro exists so that code which calls `tr!` still compiles when the `i18n`
/// feature is off. It performs **no translation**: it logs a warning naming the key
/// and returns the key itself. That makes a missing translation loud during
/// development instead of quietly rendering an empty string, but it also means the
/// returned text is a message *key*, not user-facing copy — a build without `i18n`
/// must not be shipped as a localized one.
///
/// Accepts the same three arities as the real macro (`$key`, `$key, $count`, and
/// `$key, $context, $count`) so call sites need no `cfg` of their own; the plural
/// and context arguments are ignored here.
#[cfg(not(feature = "i18n"))]
#[macro_export]
macro_rules! tr {
    ($key:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $context:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
}
#[cfg(not(feature = "i18n"))]
/// `tr_into!` appends a translation to a caller-provided buffer.
///
/// This build has no catalogue, so it logs the missing-feature warning that [`tr!`] logs and
/// appends the key. It accepts the same arities as the `i18n` build's macro so call sites need
/// no `cfg` of their own.
#[macro_export]
macro_rules! tr_into {
    ($key:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        $out.push_str($key);
    }};
    ($key:expr, $count:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        let _ = $count;
        $out.push_str($key);
    }};
    ($key:expr, $context:expr, $count:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        let _ = ($context, $count);
        $out.push_str($key);
    }};
}
/// Application lifecycle wrapper and type-safe widget handles (not available in mini mode).
///
/// Same condition as `crate::json`: `full_widgets`.
#[cfg(full_widgets)]
pub mod app;
/// Index-based widget registry for runtime lookup.
pub mod index;
#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;
#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;
/// Initialize global platform and i18n subsystems.
///
/// One function for every profile (BLUE15 rule #58): the branch that used to be
/// a `cfg`-gated pair now asks [`platform::profile`], so adding a profile does
/// not mean adding a copy of this function.
pub fn init() {
    trace_runtime_route("init");
    platform::profile::runtime_init();
    platform::profile::init_optional_subsystems();
    // One-time menu policy probe, at startup rather than on the first menu draw.
    #[cfg(feature = "advanced-widgets")]
    crate::menu_config::init();
}
/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    platform::profile::runtime_run();
}
/// Request platform event loop shutdown, from any thread.
///
/// Returns whether this call put the session on a path to ending: `true` means either a loop in
/// this process is running its frames — so it reads the request at its next one and stops,
/// whatever thread this was called from — or the backend performed its own stop from here.
/// `false` means neither: no loop has driven a frame yet and this thread could not stop one, so
/// nothing is reading the request right now. A host that receives `false` and needs the session
/// over immediately must arrange that itself; the request stays recorded, so a loop started
/// later honours it at its first frame.
///
/// # Why this works from a thread the toolkit would refuse
///
/// No toolkit accepts a stop from an arbitrary thread (`-[NSApplication stop:]` is
/// main-thread-only, `gtk_main_quit` walks a global main-loop stack only the loop's thread may
/// touch, `PostQuitMessage` is thread-affine). Recording the request is one atomic store, which
/// every thread may do, and the loop's own frame tick performs the toolkit's stop — every loop
/// this library owns calls `drive_frame` on the thread that owns it. See
/// `platform::stop` for that seam, and [`platform::Platform::quit`] for
/// the backend-level "perform it now" half.
///
/// This is the entry a host should use when it is **not** driving the loop: a smoke test that
/// has to end its own session, CI, or a scripted demo. It is also what makes such a host's exit
/// distinguishable from a killed process, because the loop returns normally.
pub fn quit() -> bool {
    trace_runtime_route("quit");
    platform::profile::runtime_quit()
}
/// Logs the resolved profile/backend/route when `RUST_WIDGETS_TRACE_RUNTIME=1`.
///
/// # Why this writes to stderr as well as the log
///
/// `log::info!` goes to the `log` facade, and this crate installs no logger on
/// desktop builds (`src/platform/android_jni.rs` does, via logcat, and that is the
/// only one). A program that has not installed one therefore saw **nothing** when it
/// asked for the trace, so the runtime audit BLUE15 #55 requires could not actually
/// be performed: `RUST_WIDGETS_TRACE_RUNTIME=1` printed an empty line and the route
/// stayed unverified. Writing the same record to stderr when the variable is set
/// makes the audit work out of the box, while the `log` record keeps the event
/// available to a host that does install a logger. The stderr write is opt-in — the
/// variable has to be exactly `1` — so a normal run prints nothing.
pub(crate) fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() != Some("1") {
        return;
    }

    let line = format!(
        "[rust_widgets.runtime] stage={} profile={} backend={} route={} host={}",
        stage,
        platform::profile::profile_name(),
        platform::platform_facts().backend_name(),
        platform::profile::route_name(),
        platform::profile::host_name()
    );
    log::info!("{line}");
    eprintln!("{line}");
}
// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting a platform instance

/// Resolve the backend that should create a widget of this kind.
///
/// # Why this exists
///
/// A widget kind may map onto a real platform primitive (a `Button` on Win32/
/// AppKit/GTK) or have no primitive at all (`Chart`, `CodeEditor`, …), in which
/// case the platform layer supplies the surface. **Which of the two happens is a
/// backend decision and must not leak to callers**: this function is the single
/// place the creation path asks for it, so the choice cannot drift between call
/// sites (see `control_backend::dispatcher`).
///
/// On a profile without an OS runtime (`mini`, `embedded`) the custom state
/// backend answers instead, so the same call works everywhere.
#[cfg(not(alloc_frugal))]
fn backend_for_kind(kind: widget::WidgetKind) -> &'static dyn control_backend::ControlBackend {
    control_backend::get_control_backend_for_widget(kind)
}

// ── Absent kinds do not need a stand-in ──
//
// `mini`/`embedded` compile out `MenuBar`, `Menu`, `ToolBar`, `StatusBar`,
// `ListView`, the dialogs and their `MessageBox`. The `create_*` functions that
// route on them must still **exist** in every profile so the public surface does
// not vary (principle #53), and the way that is expressed here is a single
// `#[cfg(widgets_unstripped)]` gate on the body plus an honest failure otherwise.
//
// # The dead layer this replaced
//
// There used to be nine `KIND_*` const pairs, each naming the variant under
// `widgets_unstripped` and substituting `WidgetKind::Panel` otherwise, all fed
// into `backend_for_kind(...)`. **That value was never read.**
// `get_control_backend_for_widget` discards its argument (there is one creation
// mechanism, so there is nothing to resolve), and every backend `create_*`
// hardcodes the kind it mounts (`create_menu_bar` mounts `WidgetKind::MenuBar`
// inside `control_backend/custom`). So the stand-in never reached a widget, and
// repainting its gate — to `widgets_unstripped`, to `full_widgets`, to anything —
// changed nothing observable. A prior round recorded a `KIND_LIST_VIEW` gate fix
// as a behaviour change; it was not one.
//
// The reachable defect was the opposite of what the alias pair implied: on `mini`
// and `embedded` the body still called `create_menu_bar`, which the custom backend
// used to answer with a valid id addressing **nothing**, so a caller on those
// profiles held an id that failed every later call. The body is now gated, so those
// profiles get `0` — the documented "no such control here" answer, matching what
// `tests/kind_alias_gate_test.rs` pins for the profiles that do ship the control.

/// The id returned when the running profile does not ship the requested control.
///
/// Not `Panel`'s id, and not any id that would address a different control: a
/// caller that receives `0` can test for it, whereas a valid id addressing the
/// wrong thing cannot be detected at all. `widget::runtime` documents `0` as "no
/// widget", and the C ABI already reports it the same way.
///
/// Gated `not(widgets_unstripped)` because that is exactly where it is consumed:
/// on a profile that ships the whole widget set every `create_*` reaches its real
/// constructor and the fallback branch is compiled out.
#[cfg(all(not(alloc_frugal), not(widgets_unstripped)))]
const NO_SUCH_CONTROL: crate::core::ObjectId = 0;

/// Create a top-level window with specified title and geometry.
///
/// The window is created **hidden**. [Show it](show_widget) after its children exist (or at any
/// later time); creating it alone is not a request to map it, so a caller that never shows it
/// gets no window on screen.
///
/// # Example
/// ```
/// let window_id = rust_widgets::create_window("My App", 100, 100, 800, 600);
/// // Build the window's controls first, so the first frame includes them.
/// rust_widgets::show_widget(window_id);
/// ```
#[cfg(not(alloc_frugal))]
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Window).create_window(title, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a button control as a child of specified parent.
///
/// The backend decides whether this becomes a platform button or is painted by
/// the platform's custom surface; callers get a handle either way.
pub fn create_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Button).create_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a check-box. The backend decides whether it is a platform control
/// or is painted by the platform's custom surface.
pub fn create_checkbox(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::CheckBox)
        .create_checkbox(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a single-line text editor. The backend chooses how it is hosted.
pub fn create_line_edit(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::LineEdit)
        .create_line_edit(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a read-only text label. The backend chooses how it is hosted.
pub fn create_label(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Label).create_label(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a radio button. The backend chooses how it is hosted.
pub fn create_radio_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::RadioButton)
        .create_radio_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a slider. The backend chooses how it is hosted.
pub fn create_slider(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Slider).create_slider(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a progress bar. The backend chooses how it is hosted.
pub fn create_progress_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ProgressBar)
        .create_progress_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a combo box. The backend chooses how it is hosted.
pub fn create_combo_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ComboBox).create_combo_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list box. The backend chooses how it is hosted.
pub fn create_list_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ListBox).create_list_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a panel (a container surface). The backend chooses how it is hosted.
pub fn create_panel(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    // `WidgetKind::Panel` is the closest always-available kind; on profiles where
    // a dedicated panel kind exists the routing table answers for it.
    backend_for_kind(widget::WidgetKind::Panel).create_panel(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a message box dialog as a child of specified parent.
///
/// Creates a message box. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the message box (`mini`,
/// `embedded`). Callers must treat `0` as "no control was created" rather than
/// using it as a parent: see the note above `NO_SUCH_CONTROL`.
pub fn create_message_box(
    parent: crate::core::ObjectId,
    title: &str,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MessageBox)
            .create_message_box(parent, title, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, title, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a file dialog as a child of specified parent.
///
/// Creates a file dialog. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the file dialog.
pub fn create_file_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FileDialog)
            .create_file_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a color dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the color dialog.
pub fn create_color_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ColorDialog)
            .create_color_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a font dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the font dialog.
pub fn create_font_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FontDialog)
            .create_font_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Creates a spin box. The backend chooses how it is hosted.
pub fn create_spin_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::SpinBox).create_spin_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list view. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the item view.
pub fn create_list_view(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ListView).create_list_view(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a scroll area. The backend chooses how it is hosted.
#[cfg(not(alloc_frugal))]
pub fn create_scroll_area(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ScrollArea).create_scroll_area(parent, x, y, width, height)
}

/// Creates a tab widget. The backend chooses how it is hosted.
///
/// A tab widget's pages are child ids declared with
/// [`app::TabWidgetHandle::add_tab`](crate::app::TabWidgetHandle::add_tab), not the
/// generic `Layout` machinery: a page is selected, not placed in a track.
///
/// # Why the gate is `widgets_unstripped`
///
/// This function names [`widget::WidgetKind::TabWidget`], which exists only where the
/// widget set was not stripped (`widgets_unstripped`, i.e. not `mini` **and** not
/// `embedded`). Gating the constructor with `not(alloc_frugal)` was narrower than the
/// variant it references: `embedded` has `alloc_frugal = false` and
/// `widgets_unstripped = false`, so the constructor compiled and the variant did not,
/// making the profile fail with `no variant ... named TabWidget`. The constructor and
/// the variant must share the same predicate (principle #47).
#[cfg(widgets_unstripped)]
pub fn create_tab_widget(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::TabWidget).create_tab_widget(parent, x, y, width, height)
}

/// Creates a splitter. Its panes are child ids added through
/// [`app::SplitterHandle::add_pane`](crate::app::SplitterHandle::add_pane); ratios are the
/// splitter's own content model, not a generic layout.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Splitter` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_splitter(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Splitter).create_splitter(parent, x, y, width, height)
}

/// Creates a grid widget — a visual row/column matrix, distinct from the generic
/// `layout::GridLayout` placement engine.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Grid` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_grid_widget(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Grid).create_grid(parent, x, y, width, height)
}

/// Creates a generic dialog. Its content child is set with
/// [`app::DialogHandle::set_content_widget`](crate::app::DialogHandle::set_content_widget);
/// the dialog owns the chrome and the content owns its own layout.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Dialog` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_dialog(
    parent: crate::core::ObjectId,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Dialog).create_dialog(parent, title, x, y, width, height)
}
/// Mounts a widget onto a surface supplied by the host.
///
/// The host platform supplies a window and a drawing surface; the library paints
/// the widget through [`widget::Draw`]. Which surface hosts it — a child window,
/// a drawing area, a view — is decided inside `src/platform/` and never named
/// here; callers only need to know whether the backend can host widgets, which
/// [`supports_surfaces`] answers.
///
/// `id` must already be registered in `widget::runtime`. Prefer the
/// higher-level `app::WindowHandle::mount_surface`, which performs the
/// registration for you and reports failures as a `Result`.
///
/// Returns `false` when the backend has no surface to offer, or when it refuses
/// this particular mount. Backends that cannot host widgets log why.
#[cfg(not(alloc_frugal))]
pub fn mount_surface(
    parent: crate::core::ObjectId,
    id: crate::core::ObjectId,
    rect: crate::core::Rect,
) -> bool {
    platform::get_platform().mount_surface(parent, id, rect)
}

/// Moves and resizes a mounted surface.
#[cfg(not(alloc_frugal))]
pub fn resize_surface(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().resize_surface(id, rect)
}

/// Unmounts a surface from its window.
#[cfg(not(alloc_frugal))]
pub fn unmount_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().unmount_surface(id)
}

/// Marks a mounted surface as needing a repaint.
///
/// Returns `false` when the id is not a surface mounted on the active backend.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().invalidate_surface(id)
}

/// Asks the platform to repaint one rectangle of a mounted widget.
///
/// Returns `true` when the backend narrowed the repaint to `rect`. `false` — including
/// from a backend that never implemented this — means the caller should fall back to
/// [`invalidate_surface`], which repaints the whole widget. Backends are not required
/// to implement it, because several window toolkits offer only whole-widget
/// invalidation.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface_rect(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().invalidate_surface_rect(id, rect)
}

/// Asks the platform to repaint one rectangle, or reports that it cannot.
///
/// The alloc-frugal profile has no platform trait to forward to, so this reports
/// `false` and the caller invalidates the whole surface instead — which is what that
/// profile would have done anyway.
#[cfg(alloc_frugal)]
pub fn invalidate_surface_rect(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Removes and returns the next widget whose surface is awaiting a repaint.
///
/// # Why a host outside the process needs this
///
/// On the three desktop backends the library drives the event loop, so an
/// invalidation reaches the toolkit's own "this window is dirty" call directly. A
/// host that owns its own loop and its own drawing API — OpenHarmony (ArkUI
/// `Canvas`/XComponent), Android (`Bitmap`), iOS (`NSView`), the objc2 preview —
/// cannot be called into that way, so the backend records the stale surfaces and the
/// host pulls them. This is that pull, and it is the function
/// [`rw_take_pending_repaint`](crate::bindings) exposes to the C ABI.
///
/// # Coalescing
///
/// A widget already awaiting a repaint is not queued twice, so a burst of
/// invalidations in one frame costs the host one entry. That is what makes a drag
/// that produces hundreds of geometry updates pay for one repaint per frame rather
/// than one per update.
///
/// `None` when nothing is pending — the honest answer rather than an empty sentinel
/// id, because `0` is a valid `ObjectId` in no backend but is equally not a widget
/// this call could have returned.
#[cfg(not(alloc_frugal))]
pub fn take_pending_repaint() -> Option<crate::core::ObjectId> {
    platform::get_platform().take_pending_repaint()
}

/// The alloc-frugal profile's [`take_pending_repaint`].
///
/// Reports `None` because this profile has no widget registry and therefore keeps no
/// repaint queue to drain — the same truthful "nothing is pending" a full build
/// answers for a window with no damage. See the full build's version for why the
/// function exists at all.
#[cfg(alloc_frugal)]
pub fn take_pending_repaint() -> Option<crate::core::ObjectId> {
    None
}

/// Returns `true` when the active backend can host library-painted widgets.
#[cfg(not(alloc_frugal))]
pub fn supports_surfaces() -> bool {
    platform::get_platform().supports_surfaces()
}

/// Delivers a pointer event that arrived on a host-owned drawing surface.
///
/// # Why this exists, and why it was missing
///
/// On the three desktop backends the library drives the event loop, so a click arrives
/// through the toolkit's own callback and is routed inside it (`windows/types.rs`'
/// window procedure, `linux/platform_impl.rs`' GTK handler). A host that owns its own
/// loop — OpenHarmony's ArkUI, Android's `Activity`, iOS's touch handling, a browser page —
/// **is never called into**, so it has the opposite obligation: it must hand its touches
/// back. Until this function existed there was no way to do that.
///
/// That was not a documentation gap. `src/platform/android/status.md`,
/// `src/platform/ios/status.md` and `docs/plans/harmony_integration.md` all told a host to
/// call `rw_dispatch_pointer_event(...)`, and `grep -rn rw_dispatch_pointer_event src/
/// include/` returned nothing: the documented integration step — without which a mounted
/// widget is visible but completely inert — had no implementation at any layer.
///
/// # What `root` is, and why a hit test rather than an id
///
/// `point` is in the **window's** coordinate space, and `root` names the widget subtree to
/// resolve it in — normally the window. The library hit-tests the point against that
/// subtree ([`crate::widget::runtime::widget_at`]), so the host does not have to know which
/// widget was under the finger. That is the whole point of keeping the routing here: a host
/// that sent the event to a widget id would need the same hit test the library already owns,
/// and the two would drift.
///
/// Returns whether a widget accepted the event. `false` means the point was outside every
/// widget in `root`, or the resolved widget refused it (disabled, or blocked by a modal) —
/// never "this host cannot route", which is a distinct case the alloc-frugal profile
/// reports through [`route_pointer_event`](crate::platform::Platform::route_pointer_event).
#[cfg(not(alloc_frugal))]
pub fn dispatch_pointer_event(
    root: crate::core::ObjectId,
    event: &crate::event::Event,
    point: crate::core::Point,
) -> bool {
    platform::get_platform().route_pointer_event(root, event, point)
}

/// The alloc-frugal profile's [`dispatch_pointer_event`].
///
/// Reports `false` because this profile compiles out the widget registry, so there is no
/// subtree to hit-test and no widget that could accept the event. That is the same honest
/// "this host cannot do it" answer [`crate::platform::Platform::route_pointer_event`]'s
/// default gives, rather than a silent no-op that looks like a delivered event.
#[cfg(alloc_frugal)]
pub fn dispatch_pointer_event(
    _root: crate::core::ObjectId,
    _event: &crate::event::Event,
    _point: crate::core::Point,
) -> bool {
    false
}

/// Delivers an event straight to `widget_id`, without hit-testing.
///
/// The counterpart to [`dispatch_pointer_event`] for the cases where the host **already
/// knows** the target: a key event for the focused control, a text-input commit, a scroll
/// from a surface the host tracks itself. Routing a pointer event this way skips the
/// library's hit test, so a host that uses it for touches must do its own — which is why
/// the pointer path above exists and is the one the integration documents name.
///
/// Returns whether the widget accepted the event: `false` when the id is not a live widget, or
/// when a modal blocks it. A **disabled** control is refused by the pointer router
/// ([`dispatch_pointer_event`]) before delivery, which is where the crate's "the router skips
/// disabled controls when delivering" rule lives; this entry point delivers to whatever live id it
/// is handed, so a host that routes input straight to a control it has disabled must check its own
/// state.
#[cfg(not(alloc_frugal))]
pub fn dispatch_event(widget_id: crate::core::ObjectId, event: &crate::event::Event) -> bool {
    crate::widget::runtime::dispatch_event(widget_id, event)
}

/// The alloc-frugal profile's [`dispatch_event`].
///
/// `false` for the same reason as [`dispatch_pointer_event`]: no registry, so no widget.
#[cfg(alloc_frugal)]
pub fn dispatch_event(_widget_id: crate::core::ObjectId, _event: &crate::event::Event) -> bool {
    false
}

/// Mounts a widget object on the platform's surface, or reports why it cannot.
///
/// This is the **single implementation** of the register → mount → roll back on
/// failure sequence, used by both [`create_widget_of_kind`] and
/// [`app::WindowHandle::mount_surface`]. Keeping one
/// copy is what stops the two paths from disagreeing about ownership: on any
/// failure the widget is unregistered, so the registry never holds a widget the
/// backend is not showing.
///
/// # Resolving the parent
///
/// `parent` may be either a **window widget** (what [`create_window`] returns and
/// what `App::new_window` hands out) or a **host window** (what
/// `Platform::create_window` returns). `mount_surface` is a `Platform` method, so
/// it can only resolve the latter — the platform knows nothing of the widget
/// registry. Translating here, at the one place that already owns both the widget
/// registry and the platform handle, is what lets a caller mount controls on the
/// window it created. Without it the library's own windows could not carry the
/// library's own controls, because the two id spaces never met.
///
/// Applies the active theme to a widget that is about to be registered.
///
/// # Why this shim exists at the crate root
///
/// Both creation funnels (`mount_widget_object` here and
/// `CustomPaintControlBackend::mount_widget_of_kind`) must apply the theme, and
/// neither may carry its own `#[cfg]` — a call-site gate is exactly where the
/// condition drifts away from the module's own gate. That drift broke the
/// `embedded` build twice: once for the JSON loader, once here.
///
/// So the call is unconditional and the *body* is gated, which means the condition
/// is written once, next to the module it describes.
#[cfg(not(alloc_frugal))]
fn apply_active_theme(widget: &mut Box<dyn widget::Widget>) {
    #[cfg(device_profile)]
    {
        crate::theme::apply_active_theme(&mut **widget);
    }
    #[cfg(not(device_profile))]
    {
        // No theme module in this profile, so there is nothing to apply. Naming the
        // parameter keeps the signature identical in every build.
        let _ = widget;
    }
}

/// Completes a theme switch by re-styling the controls the switch notified (G-404).
///
/// # How a switch reaches mounted controls
///
/// Every mounted control subscribes to the theme manager's `theme_changed` signal at
/// registration (see `widget::runtime::finish_registration`). A switch — `set_theme`,
/// `set_appearance`, the high-contrast override, a system-accent refresh, a loaded-and-activated
/// theme — emits that signal, and each subscription records its control in the runtime's
/// pending-restyle set. This call drains that set: each notified control is re-merged with the
/// active theme through the same `theme::apply_active_theme` a newly created control gets, so an
/// explicit per-control style still wins (see `theme::apply`).
///
/// # Why the restyle is a drain, not a synchronous notification
///
/// The signal fires while the switching caller holds the theme mutex, and the re-merge resolves
/// theme styles — which takes that same non-reentrant mutex. Restyling inside the slot would
/// deadlock, so the slot only records intent and the merge happens here, after the manager guard
/// is released, or at the next frame (`drive_frame` step 0a) for switch paths that do not call
/// this.
///
/// # Coverage
///
/// A control subscribes exactly once, at registration, so the pending set equals the set of
/// controls mounted on the switching thread — the same set the previous full walk of the
/// registry restyled, minus the blind re-merge of controls nothing notified about.
/// `tests/theme_application_test.rs` pins the parity: mount, switch, assert restyled.
///
/// Without a preceding switch the set is empty and this is a no-op: nothing has notified, so no
/// control carries a style that disagrees with the active theme. A profile without a theme
/// module has nothing to apply and does nothing.
#[cfg(not(alloc_frugal))]
pub fn reapply_active_theme() {
    #[cfg(device_profile)]
    {
        widget::runtime::drain_pending_theme_restyle();
    }
}

/// Switches to the appearance the **device** asks for, then restyles every live widget.
///
/// # Why the two steps are one call
///
/// Choosing a theme and painting the widgets that already exist are a single user-visible outcome:
/// a host that called only the first would have the right `current_theme()` and the old palette on
/// screen until something else forced a repaint. Composing them here is what keeps "the app follows
/// the system appearance" from depending on the caller remembering the sweep.
///
/// Returns whether the appearance was recognised, so a host can tell "nothing needed doing"
/// (the device asks for the appearance already active) from "no theme declares that appearance"
/// (a packaging error worth surfacing). See
/// [`ThemeManager::follow_environment_appearance`](crate::theme::ThemeManager::follow_environment_appearance)
/// for what each [`ThemeMode`](crate::style::ThemeMode) resolves to.
///
/// # When to call it
///
/// The device preference is read through `crate::style::environment`, whose snapshot is taken once
/// per frame by `drive_frame`. A host that learns of a system appearance
/// change therefore calls this between frames — which is exactly what the environment's own docs
/// describe ("a host that changed a system setting between frames sees it take effect on the next
/// one"). A profile without a theme module has nothing to switch and reports `false`.
#[cfg(not(alloc_frugal))]
pub fn apply_environment_appearance() -> bool {
    #[cfg(device_profile)]
    {
        let changed = crate::theme::global_theme_manager().follow_environment_appearance();
        if changed {
            reapply_active_theme();
        }
        changed
    }
    #[cfg(not(device_profile))]
    {
        // No theme registry in this profile, so no appearance to follow. Reporting `false` is the
        // truthful answer rather than a fabricated "changed".
        false
    }
}

/// Returns `Ok(id)` with the widget live in the registry, or `Err(reason)`.
#[cfg(not(alloc_frugal))]
fn mount_widget_object(
    parent: crate::core::ObjectId,
    mut widget: Box<dyn widget::Widget>,
    rect: crate::core::Rect,
) -> Result<crate::core::ObjectId, widget::runtime::SurfaceMountError> {
    use widget::runtime::SurfaceMountError;

    // Apply the active theme before the widget becomes visible. This is one of the
    // two funnels every created control passes through (the other is
    // `CustomPaintControlBackend::mount_widget_of_kind`), which is what makes a
    // theme switch affect controls created through the C ABI and not only those the
    // JSON loader builds. An explicit style the caller or the constructor already
    // set is preserved; see `theme::apply`.
    apply_active_theme(&mut widget);

    // Register first: the backend looks the widget up by id on every repaint.
    let id = widget::runtime::register(widget).ok_or(SurfaceMountError::NoRegistryOnThread)?;
    widget::runtime::set_geometry(id, rect);
    // The surface is not a child in the widget tree, so this is the only record that
    // it belongs to `parent`. Cursor dispatch and repaint routing both need it.
    widget::runtime::set_surface_window(id, parent);

    let host_parent = widget::runtime::host_window_for(parent).unwrap_or(parent);

    if !platform::get_platform().mount_surface(host_parent, id, rect) {
        // Do not leave a widget stranded when the backend refused to show it.
        widget::runtime::unregister(id);
        if !supports_surfaces() {
            return Err(SurfaceMountError::UnsupportedByBackend(backend_name()));
        }
        return Err(SurfaceMountError::RejectedByBackend(backend_name()));
    }
    Ok(id)
}

/// Creates a widget of any kind — **the single, mechanism-free creation entry**.
///
/// # How this differs from the per-kind `create_*` functions
///
/// The `create_*` functions are the typo-safe spelling for the common widgets
/// (`create_button`, `create_slider`, …). This function is the general form: hand
/// Creates a widget of `kind`, painted by the library.
///
/// # What this does now
///
/// Every kind is painted by the library (BLUE15 rule #55), so there is a single
/// path: build (or accept) the `Box<dyn Widget>` and hand it to the host surface
/// through `widget::runtime`. The `Platform` layer supplies a surface to paint on,
/// not a control to map onto, so no branch here asks which mechanism to use.
///
/// # The `widget` argument
///
/// A caller may supply a pre-built object so that a widget with constructor
/// arguments the factory cannot express (`CodeEditor` with initial text, a chart
/// with series) reaches this path without a bespoke branch. When it is `None` the
/// widget factory is asked; that factory is the single place that knows every
/// kind's constructor (rule #65).
///
/// Returns `0` — a truthful "not created" — when the kind has no constructor and
/// the caller supplied no object. Callers already handle `0` for the per-kind
/// functions, so this needs no separate error channel.
#[cfg(not(alloc_frugal))]
pub fn create_widget_of_kind(
    kind: widget::WidgetKind,
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    widget: Option<Box<dyn widget::Widget>>,
) -> crate::core::ObjectId {
    let rect = crate::core::Rect::new(x, y, width, height);

    // Prefer the caller's object; otherwise build from the factory, which is the
    // only component that knows every kind's constructor.
    //
    // The gate is `full_widgets`, not `device_profile`: the factory and
    // `kind_name` both require the registry, which exists only when a device
    // profile is present *and* the widgets were not stripped. Spelling it
    // `device_profile` made the callee narrower than its caller, so a build that
    // overlapped two axis-1 features (`--features "tablet,embedded"`) failed with
    // `cannot find function kind_name` / `cannot find WidgetFactory`. `full_widgets`
    // is exactly the conjunction the body needs, so there is no narrower gate left
    // for this call site to drift away from (principle #47).
    let widget = widget.or_else(|| {
        #[cfg(all(device_profile, full_widgets))]
        {
            // Direct kind lookup: `create(&kind_name(..))` resolved the kind to a canonical
            // name (allocating a `String`), then `create` normalised that name and looked the
            // capability up again. `create_by_kind` uses the same constructor without the
            // round trip; the name path is only the fallback for a kind the registry lists by
            // name but not by kind.
            let factory = widget::WidgetFactory::shared();
            factory.create_by_kind(kind, rect, text).or_else(|| {
                let name = widget::capability::factory_name_for_kind(kind)?;
                factory.create(name, rect, text)
            })
        }
        #[cfg(not(all(device_profile, full_widgets)))]
        {
            let _ = (text, rect);
            None
        }
    });

    let Some(widget) = widget else {
        log::warn!(
            "create_widget_of_kind: no constructor for {kind:?} and no widget object was \
             supplied; returning 0"
        );
        return 0;
    };

    // A backend that has no per-widget surface has exactly one way to host a control: the
    // control backend's widget-tree path, which is also what every `create_*` function uses.
    // Sending the object to `mount_widget_object` first would register it, fail the mount,
    // unregister it and drop it — so this branch has to be taken **before** the surface path,
    // not as a recovery after it. That asymmetry is what made `create_widget_of_kind` return
    // 0 for every kind on a native desktop backend while the typed constructors succeeded.
    if !supports_surfaces() {
        // Parity with the typed constructors: a non-window control needs a live parent, or
        // the returned id would address a widget no tree can reach. The surface path got this
        // for free because `mount_surface` refuses an unknown parent; the widget-tree path has
        // to say it explicitly.
        if kind != widget::WidgetKind::Window
            && (parent == 0 || !widget::runtime::is_mounted(parent))
        {
            log::warn!(
                "create_widget_of_kind: refusing to create {kind:?} under parent {parent}, which \
                 addresses no live container; returning 0"
            );
            return 0;
        }
        let id = control_backend::get_control_backend().mount_widget_box(widget, Some(parent), 0);
        if id == 0 {
            log::warn!(
                "create_widget_of_kind: {kind:?} could neither be hosted on a platform \
                 surface nor adopted by the widget-tree backend; returning 0"
            );
        }
        return id;
    }

    match mount_widget_object(parent, widget, rect) {
        Ok(id) => id,
        Err(error) => {
            log::warn!("create_widget_of_kind: cannot host {kind:?}: {error}");
            0
        }
    }
}

/// Stub for mini mode (no platform runtime, no windows).
#[cfg(alloc_frugal)]
pub fn mount_surface(
    _parent: crate::core::ObjectId,
    _id: crate::core::ObjectId,
    _rect: crate::core::Rect,
) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn resize_surface(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn unmount_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn invalidate_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn supports_surfaces() -> bool {
    false
}

/// Show a widget by its object id.
///
/// Routed through the control backend, which is the one mechanism that owns the
/// control (BLUE15 #55): the platform supplies the surface, not the control.
#[cfg(not(alloc_frugal))]
pub fn show_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, true);
}
/// Hide a widget by its object id.
#[cfg(not(alloc_frugal))]
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, false);
}

/// Announce that a widget was **closed**, by emitting its
/// [`BaseWidget::closed`](crate::widget::BaseWidget::closed) signal **and** its published `closed`
/// signal when the control declares one.
///
/// # Why this is the join
///
/// A closeable control's own `close`/`dismiss` path and the handle layer's `close` are two entry
/// points to one lifecycle fact. Both must reach the same signal, or a handler registered through
/// one route misses a close produced by the other.
///
/// # Why both signals are emitted
///
/// A control may publish `closed` through a signal of its own (`Window::closed`) in addition to the
/// base one. The JSON/designer route binds `events:{"closed":…}` to the control's **published**
/// signal via `event_signal_dyn`, while the handle layer's `on_close` connects to `base.closed`. If
/// this generic close path emitted only one of them, whichever route used the other would stay
/// silent — which is exactly how a `closed` handler came to fire for one route and not the other.
/// Emitting both means either listener fires from whichever path closed the control.
///
/// Returns `true` when a mounted widget received it. A `false` means the id addresses no live
/// widget — the same honest answer [`hide_widget`] gives by doing nothing observable.
#[cfg(not(alloc_frugal))]
pub fn close_widget(widget_id: crate::core::ObjectId) -> bool {
    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().closed.emit();
        // The published `closed`, when it is a signal of the control's own rather than the base one
        // already emitted above. `unit` refs over `base.closed` are the same signal, so re-emitting
        // would just run the base slots twice; comparing identities is not possible through the
        // erased ref, so this relies on each control publishing a *distinct* field (as `Window`
        // does). A control whose published `closed` is already `base.closed` is unaffected by the
        // extra emit because `Signal::emit` on the same signal during its own emit pass skips the
        // slots already on the stack (see `Signal::emit`'s re-entrancy rule).
        if let Some(reference) = widget.event_signal_dyn("closed") {
            reference.emit_unit();
        }
    })
    .is_some()
}
/// Set geometry of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_geometry(
    widget_id: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    control_backend::get_control_backend().set_widget_geometry(widget_id, x, y, width, height);
}
/// Read a widget's rectangle, or `None` when the id addresses nothing.
#[cfg(not(alloc_frugal))]
pub fn widget_geometry(widget_id: crate::core::ObjectId) -> Option<(i32, i32, u32, u32)> {
    control_backend::get_control_backend().get_widget_geometry(widget_id)
}
/// Set text of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    control_backend::get_control_backend().set_widget_text(widget_id, text);
}
/// Get text of a widget.
#[cfg(not(alloc_frugal))]
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    control_backend::get_control_backend().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
#[cfg(not(alloc_frugal))]
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    control_backend::get_control_backend().set_widget_visible(widget_id, visible);
}
/// Check if a widget is visible.
#[cfg(not(alloc_frugal))]
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_visible(widget_id)
}

/// Set a widget's primary numeric value (slider, progress bar, spin box, ...).
///
/// Returns `false` when the widget exposes no numeric value, so callers never
/// mistake "unsupported" for "set to 0".
#[cfg(not(alloc_frugal))]
pub fn set_widget_value(widget_id: crate::core::ObjectId, value: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["value"], value)
}

/// Read a widget's primary numeric value.
#[cfg(not(alloc_frugal))]
pub fn widget_value(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["value"])
}

/// Set a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn set_widget_range(widget_id: crate::core::ObjectId, min: f64, max: f64) -> bool {
    let names: &[&str] = &["minimum", "min", "min_value"];
    let max_names: &[&str] = &["maximum", "max", "max_value"];
    widget::capability::widget_access::write_number(widget_id, names, min)
        && widget::capability::widget_access::write_number(widget_id, max_names, max)
}

/// Read a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn widget_range(widget_id: crate::core::ObjectId) -> Option<(f64, f64)> {
    let min = widget::capability::widget_access::read_number(
        widget_id,
        &["minimum", "min", "min_value"],
    )?;
    let max = widget::capability::widget_access::read_number(
        widget_id,
        &["maximum", "max", "max_value"],
    )?;
    Some((min, max))
}

/// Set a widget's selection index (combo box, list box, tab widget).
///
/// `None` clears the selection, which is what these controls store as `Null` —
/// distinct from selecting index 0.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selected_index(widget_id: crate::core::ObjectId, index: Option<usize>) -> bool {
    let names: &[&str] = &["selected_index", "current_index", "current_row", "active_index"];
    let value = match index {
        Some(index) => crate::widget::capability::CapabilityValue::UInt(index as u64),
        None => crate::widget::capability::CapabilityValue::Null,
    };
    widget::capability::widget_access::write_first(widget_id, names, &value)
}

/// Read a widget's selection index.
#[cfg(not(alloc_frugal))]
pub fn widget_selected_index(widget_id: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(
        widget_id,
        &["selected_index", "current_index", "current_row", "active_index"],
    )
}

/// Set a widget's checked state (check box, radio button, toggle button).
#[cfg(not(alloc_frugal))]
pub fn set_widget_checked(widget_id: crate::core::ObjectId, checked: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["checked"],
        &crate::widget::capability::CapabilityValue::Bool(checked),
    )
}

/// Read a widget's checked state, or `None` when it is not checkable.
#[cfg(not(alloc_frugal))]
pub fn is_widget_checked(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["checked"])
}

/// Set a widget's increment step (slider, spin box, scroll bar).
#[cfg(not(alloc_frugal))]
pub fn set_widget_step(widget_id: crate::core::ObjectId, step: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["single_step", "step"], step)
}

/// Read a widget's increment step.
#[cfg(not(alloc_frugal))]
pub fn widget_step(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["single_step", "step"])
}

/// Set a progress-style widget's indeterminate (busy) state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_indeterminate(widget_id: crate::core::ObjectId, indeterminate: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["indeterminate"],
        &crate::widget::capability::CapabilityValue::Bool(indeterminate),
    )
}

/// Read a progress-style widget's indeterminate state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_indeterminate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["indeterminate"])
}

/// Set a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_read_only(widget_id: crate::core::ObjectId, read_only: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["read_only"],
        &crate::widget::capability::CapabilityValue::Bool(read_only),
    )
}

/// Read a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_read_only(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["read_only"])
}

/// Set a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn set_widget_max_length(widget_id: crate::core::ObjectId, max_length: u32) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["max_length"],
        &crate::widget::capability::CapabilityValue::UInt(max_length as u64),
    )
}

/// Read a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn widget_max_length(widget_id: crate::core::ObjectId) -> Option<u32> {
    widget::capability::widget_access::read_index(widget_id, &["max_length"])
        .and_then(|v| u32::try_from(v).ok())
}

/// Apply or clear a window state (maximised, minimised, full-screen, ...).
///
/// Returns `false` when the id is not a window or the backend cannot honour the
/// state on its toolkit.
#[cfg(not(alloc_frugal))]
pub fn set_window_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
    on: bool,
) -> bool {
    platform::get_platform().set_window_state(widget_id, flag, on)
}

/// Read a window state, or `None` when the id is not a window.
#[cfg(not(alloc_frugal))]
pub fn is_window_in_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
) -> Option<bool> {
    platform::get_platform().is_window_in_state(widget_id, flag)
}

/// Set a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn set_window_min_size(widget_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    platform::get_platform().set_window_min_size(widget_id, width, height)
}

/// Read a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn window_min_size(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    platform::get_platform().window_min_size(widget_id)
}

/// Set a window's icon from a file path.
#[cfg(not(alloc_frugal))]
pub fn set_window_icon(widget_id: crate::core::ObjectId, path: &str) -> bool {
    platform::get_platform().set_window_icon(widget_id, path)
}

/// Read a window's icon path, if one was set.
#[cfg(not(alloc_frugal))]
pub fn window_icon(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().window_icon(widget_id)
}

/// Set a text entry's selection range as `(start, end)` character offsets.
///
/// # Current support
///
/// No control publishes a selection range as a property (the text controls track
/// it internally and expose `cursor_position`), so this reports `false` rather
/// than pretending the range was applied. Routing it through the property
/// contract means publishing the property on the text controls is the only change
/// needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selection(widget_id: crate::core::ObjectId, start: u32, end: u32) -> bool {
    let names: &[&str] = &["selection"];
    // The pair travels as the historical "start,end" spelling used by the text
    // controls' own setter, so a control that publishes `selection` parses one
    // string rather than needing two properties kept in step.
    widget::capability::widget_access::write_first(
        widget_id,
        names,
        &crate::widget::capability::CapabilityValue::String(format!("{start},{end}")),
    )
}

/// Read a text entry's selection range, or `None` when nothing is selected.
#[cfg(not(alloc_frugal))]
pub fn widget_selection(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let raw = widget::capability::widget_access::read_text(widget_id, &["selection"])?;
    let (start, end) = raw.split_once(',')?;
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?))
}

/// Set a text entry's placeholder (cue) text.
#[cfg(not(alloc_frugal))]
pub fn set_widget_placeholder(widget_id: crate::core::ObjectId, text: &str) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["placeholder_text", "placeholder"],
        &crate::widget::capability::CapabilityValue::String(text.to_string()),
    )
}

/// Read a text entry's placeholder text.
#[cfg(not(alloc_frugal))]
pub fn widget_placeholder(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["placeholder_text", "placeholder"])
}

/// Set a text entry's echo mode.
///
/// # Current support
///
/// No control publishes an echo property yet, so this reports `false` on every
/// widget rather than pretending the mode was applied. It is routed through the
/// property contract — the place a control would express it — so that adding the
/// property to the text controls is the only change needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_echo_mode(widget_id: crate::core::ObjectId, mode: platform::EchoMode) -> bool {
    let token = match mode {
        platform::EchoMode::Normal => "normal",
        platform::EchoMode::Password => "password",
        platform::EchoMode::NoEcho => "no_echo",
    };
    widget::capability::widget_access::write_first(
        widget_id,
        &["echo_mode"],
        &crate::widget::capability::CapabilityValue::String(token.to_string()),
    )
}

/// Read a text entry's echo mode.
#[cfg(not(alloc_frugal))]
pub fn widget_echo_mode(widget_id: crate::core::ObjectId) -> Option<platform::EchoMode> {
    match widget::capability::widget_access::read_text(widget_id, &["echo_mode"])?.as_str() {
        "normal" => Some(platform::EchoMode::Normal),
        "password" => Some(platform::EchoMode::Password),
        "no_echo" => Some(platform::EchoMode::NoEcho),
        _ => None,
    }
}

/// Apply a slider's creation-time orientation.
#[cfg(not(alloc_frugal))]
pub fn set_slider_orientation(
    widget_id: crate::core::ObjectId,
    orientation: crate::core::Orientation,
) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["orientation"],
        &crate::widget::capability::CapabilityValue::String(
            widget::capability::orientation_to_str(orientation).to_string(),
        ),
    )
}

/// Read a slider's orientation.
#[cfg(not(alloc_frugal))]
pub fn slider_orientation(widget_id: crate::core::ObjectId) -> Option<crate::core::Orientation> {
    let token = widget::capability::widget_access::read_text(widget_id, &["orientation"])?;
    widget::capability::expect_orientation(crate::widget::capability::CapabilityValue::String(
        token,
    ))
    .ok()
}

/// Set a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn set_widget_tristate(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["tristate_enabled"],
        &crate::widget::capability::CapabilityValue::Bool(enabled),
    )
}

/// Read a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn is_widget_tristate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["tristate_enabled"])
}

/// Put a radio button into a named mutually-exclusive group.
///
/// An empty group clears the membership, which these controls store as `Null`.
#[cfg(not(alloc_frugal))]
pub fn set_widget_group(widget_id: crate::core::ObjectId, group: &str) -> bool {
    let value = if group.is_empty() {
        crate::widget::capability::CapabilityValue::Null
    } else {
        crate::widget::capability::CapabilityValue::String(group.to_string())
    };
    widget::capability::widget_access::write_first(widget_id, &["group_id", "group"], &value)
}

/// Read a radio button's group name.
#[cfg(not(alloc_frugal))]
pub fn widget_group(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["group_id", "group"])
}

/// Set a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn set_widget_scroll_position(widget_id: crate::core::ObjectId, x: i32, y: i32) -> bool {
    let x_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_x"],
        x as f64,
    );
    let y_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_y"],
        y as f64,
    );
    x_ok && y_ok
}

/// Read a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn widget_scroll_position(widget_id: crate::core::ObjectId) -> Option<(i32, i32)> {
    let x = widget::capability::widget_access::read_number(widget_id, &["scroll_position_x"])?;
    let y = widget::capability::widget_access::read_number(widget_id, &["scroll_position_y"])?;
    Some((x as i32, y as i32))
}
// ComboBox operations
/// Appends an item with the given text to a combo box.
///
/// Returns `false` when the backend refuses — which includes being handed an id
/// that is not a combo box.
#[cfg(not(alloc_frugal))]
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().combo_box_add_item(combo_box, text)
}
/// Removes every item from a combo box, leaving it empty and with nothing
/// selected. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().combo_box_clear_items(combo_box)
}
/// Selects the combo box item at `index`.
///
/// Routed through the property contract as the `current_index` property, so this
/// reports `false` when the widget does not publish that property. An
/// out-of-range `index` is **not** reliably reported as a failure — read
/// [`combo_box_current_index`] back to confirm what took effect.
#[cfg(not(alloc_frugal))]
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        combo_box,
        &["current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The combo box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(combo_box, &["current_index"])
}
/// How many items a combo box holds.
///
/// Reports `0` for an unknown id as well as for a genuinely empty list, so it
/// cannot be used to tell "empty" from "not a combo box".
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(combo_box, &["item_count"]).unwrap_or(0)
}
/// The text of the combo box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().combo_box_item_text(combo_box, index)
}
// ListBox operations
/// Appends an item with the given text to a list box. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().list_box_add_item(list_box, text)
}
/// Removes the list box item at `index`, shifting later items down.
///
/// Returns `false` when the backend refuses. As with the combo box, an
/// out-of-range index is not reliably distinguished from a refusal — re-read
/// [`list_box_item_count`] to check.
#[cfg(not(alloc_frugal))]
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    control_backend::get_control_backend().list_box_remove_item(list_box, index)
}
/// Removes every item from a list box. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().list_box_clear_items(list_box)
}
/// Selects the list box item at `index`.
///
/// Tries the `current_row` property first and falls back to `current_index`,
/// because both spellings are published by list controls in this library.
/// Returns `false` when neither is published; an out-of-range `index` is not
/// reliably reported, so read [`list_box_current_index`] back to confirm.
#[cfg(not(alloc_frugal))]
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        list_box,
        &["current_row", "current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The list box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(list_box, &["current_row", "current_index"])
}
/// How many items a list box holds. Reports `0` for an unknown id as well as for
/// an empty list.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(list_box, &["item_count"]).unwrap_or(0)
}
/// The text of the list box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().list_box_item_text(list_box, index)
}
// Event polling
/// Takes the id of the next widget that has been activated, or `None` when no
/// activation is queued.
///
/// This is a queue, not a state: each successful call removes one event, so a
/// loop calling it until `None` drains the pending activations. Use
/// [`poll_widget_trigger_event`] when the kind of activation matters.
#[cfg(not(alloc_frugal))]
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_widget_triggered()
}
/// Takes the next queued widget activation together with what kind it was, or
/// `None` when the queue is empty. Drains the same queue as
/// [`poll_widget_triggered`].
#[cfg(not(alloc_frugal))]
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    control_backend::get_control_backend().poll_widget_trigger_event()
}

/// Blocks the calling thread until the framework signals work, or `timeout` elapses.
///
/// # Why hosts need this
///
/// The trigger queues are pull-based ([`poll_menu_triggered`],
/// [`poll_widget_trigger_event`]), and a host that drains them used to sleep a fixed
/// interval between polls. That cost up to one interval of latency and woke the thread
/// at that rate even while the window was still. Enqueuing a trigger (and every event
/// post) now wakes the framework's wait channel, so a host can block here and be
/// scheduled as soon as work exists. The timeout is the backstop for state that has no
/// wake source, such as a client-size change observed by polling.
///
/// Returns `true` when a wake was observed, `false` on timeout.
#[cfg(not(alloc_frugal))]
pub fn wait_for_host_events(timeout: std::time::Duration) -> bool {
    let observed = platform::frame_wait::generation();
    platform::frame_wait::wait_after(observed, timeout);
    platform::frame_wait::generation() != observed
}

/// Delivers every queued widget trigger, in order, until the queue is empty.
///
/// Returns the number of events consumed.
///
/// # Why this lives at the crate root rather than in `crate::app`
///
/// Every platform backend's event loop needs to call it — GTK's timeout, the Win32
/// message pass, Wayland's idle step, Android's polling loop — and `crate::app` is gated
/// `full_widgets` while the trigger queue is available to every profile. A backend that
/// called `crate::app::drain_triggers` therefore compiled on `desktop` and failed on
/// `mini` with `cannot find 'app' in 'crate'`, which is the same class of defect as a call
/// into a gated `create_*`: one unpicked profile hides it completely
/// (`tools/check_profiles.sh` runs the profile matrix; a `desktop` build never sees it).
///
/// The trigger queue is not a widget-set feature — it is how a backend reports "the user
/// resized the window" — so the drain sits beside the queue it drains, at the widest gate
/// its dependency has.
///
/// # Why it is defined in every profile, not gated
///
/// The queue exists in every profile except `alloc_frugal` (`mini`). Gating the function
/// would mean gating each of the **eleven** backend call sites, and a gate that has to be
/// repeated eleven times is a gate that will be forgotten in one of them — which is
/// exactly how this function came to be missing from the crate root in the first place.
/// Defining it unconditionally moves the condition to one place, and `alloc_frugal`
/// answers `0` because there is genuinely nothing to drain there.
///
/// # Termination
///
/// Drains until the queue reports empty, so a tick's cost is bounded by the number of
/// events that arrived since the last tick. A backend that produced events faster than it
/// consumed them would starve its own loop, which is why the queue is drained to empty
/// rather than to a fixed count: "drained" is the only state from which the next tick's
/// backlog is knowable.
/// Dispatches **every** queued trigger belonging to `widget_id`, and leaves other widgets' queued.
///
/// # Why this exists next to [`drain_triggers`]
///
/// [`drain_triggers`] is the host's drain: it takes the front of the **process-wide** queue until it is
/// empty, which is what a single-threaded event loop wants. A test wants something narrower -- it
/// drives one widget and needs that widget's events delivered without its result depending on what
/// another test queued first. The queue is process-wide and the test runner shares a thread pool, so
/// `drain_triggers` gives no such guarantee: a test can dispatch dozens of other tests' events and
/// never reach its own.
///
/// This removes and dispatches only the events whose `widget_id` matches, so it neither consumes nor
/// discards anyone else's work, and it preserves the order among one widget's own events. It is one
/// widget's share of the same drain, not a second implementation of it: the dispatch call is the one
/// [`drain_triggers`] makes, and the queue operation is the targeted pop beside the FIFO one.
///
/// Returns how many events it dispatched.
#[cfg(not(alloc_frugal))]
pub fn drain_widget_triggers_for(widget_id: crate::core::ObjectId) -> usize {
    let mut dispatched = 0usize;
    while let Some(event) =
        control_backend::get_control_backend().pop_widget_trigger_event_for(widget_id)
    {
        #[cfg(full_widgets)]
        {
            app::dispatch_trigger(event.widget_id, event.kind);
        }
        #[cfg(not(full_widgets))]
        {
            // No router in this profile. Popping it above is what removes it from the queue; naming
            // it keeps the binding exercised in every build.
            let _ = (event.widget_id, event.kind);
        }
        dispatched += 1;
    }
    dispatched
}

/// Drain the whole trigger queue, dispatching each event to its widget.
///
/// This is the host's entry point for step 1 of a frame: everything the backend has
/// reported since the last call is delivered, and the return value is how many events
/// that was.
///
/// # Why this is in the crate root rather than in `app`
///
/// It used to live in `app`, which is gated on `full_widgets`. A backend that
/// called `crate::app::drain_triggers` therefore compiled on `desktop` and failed on
/// `mini` with `cannot find 'app' in 'crate'`, which is the same class of defect as a call
/// into a gated `create_*`: one unpicked profile hides it completely
/// (`tools/check_profiles.sh` runs the profile matrix; a `desktop` build never sees it).
///
/// The trigger queue is not a widget-set feature — it is how a backend reports "the user
/// resized the window" — so the drain sits beside the queue it drains, at the widest gate
/// its dependency has.
///
/// # Why it is defined in every profile, not gated
///
/// The queue exists in every profile except `alloc_frugal` (`mini`). Gating the function
/// would mean gating each of the **eleven** backend call sites, and a gate that has to be
/// repeated eleven times is a gate that will be forgotten in one of them — which is
/// exactly how this function came to be missing from the crate root in the first place.
/// Defining it unconditionally moves the condition to one place, and `alloc_frugal`
/// answers `0` because there is genuinely nothing to drain there.
///
/// # Termination
///
/// Drains until the queue reports empty, so a tick's cost is bounded by the number of
/// events that arrived since the last tick. A backend that produced events faster than it
/// consumed them would starve its own loop, which is why the queue is drained to empty
/// rather than to a fixed count: "drained" is the only state from which the next tick's
/// backlog is knowable.
pub fn drain_triggers() -> usize {
    #[cfg(alloc_frugal)]
    {
        // This profile has no trigger queue, so there is nothing queued and nothing to
        // deliver. `0` is the truthful count, not a failure.
        0
    }
    #[cfg(not(alloc_frugal))]
    {
        // Only a build with a router can dispatch, so only that build counts. Naming the
        // two configurations separately keeps `mut` off the binding that never moves —
        // the compiler then checks exactly the profile-conditional fact, instead of a
        // `#[allow(unused_mut)]` suppressing it in one profile and the other profile's
        // correctness going unchecked.
        #[cfg(full_widgets)]
        let mut dispatched = 0usize;
        #[cfg(not(full_widgets))]
        let dispatched = 0usize;
        while let Some(event) = poll_widget_trigger_event() {
            #[cfg(full_widgets)]
            {
                app::dispatch_trigger(event.widget_id, event.kind);
                dispatched += 1;
            }
            #[cfg(not(full_widgets))]
            {
                // No router in this profile: `app` is gated `full_widgets`, so there is no
                // `dispatch_trigger` to hand the event to. The event is still **removed**
                // from the queue (the `poll` above did that), which is the part the queue
                // owes its callers, and it is *not* counted as dispatched.
                //
                // # Why the count must stay `0` here
                //
                // This used to increment `dispatched` unconditionally, so `embedded` —
                // which is a stripped profile with the trigger queue present but no router
                // — reported `1` for a `Resized` event that reached no layout. The return
                // value is the host's only signal that dispatch happened, and a count that
                // is indistinguishable from a real delivery is the "reported success for
                // something that did not happen" shape this crate names elsewhere. The
                // honest answer for a profile with no router is "I consumed it and
                // delivered nothing", i.e. zero, and the log line says which.
                log::debug!(
                    "drain_triggers: consumed a {:?} trigger for widget {} in a build with no \
                     widget router (profile has no device class); nothing was dispatched",
                    event.kind,
                    event.widget_id
                );
                let _ = (event.widget_id, event.kind);
            }
        }
        dispatched
    }
}
/// Queues an activation of `widget_id` as if the user had performed it, for
/// tests and for driving the UI from outside the event loop.
///
/// Returns `false` when the backend will not accept the injected event, in
/// which case nothing is queued and no later poll will report it.
#[cfg(not(alloc_frugal))]
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    control_backend::get_control_backend().inject_widget_trigger_event(widget_id, kind)
}
/// Reports that a container's client area became `width` by `height`, and queues a
/// `Resized` trigger so a host re-runs its layout.
///
/// # Who calls this
///
/// The platform backends call it from their window-resize callback (GTK
/// `size-allocate`, Win32 `WM_SIZE`, AppKit `windowDidResize:`), and a host that owns
/// its event loop may call it for a resize it learns about by other means. It is the
/// one entry point for "the user resized this window", which no `set_widget_geometry`
/// call covers — the program was not the one changing the size.
///
/// The reported size is retained (see [`window_client_size`]) and a
/// [`WidgetTriggerKind::Resized`] event is queued. The two are deliberately separate:
/// the event is consumed by polling, while the size keeps answering afterwards.
///
/// Returns `false` when `window_id` addresses nothing, so a stale id cannot inject a
/// phantom resize.
#[cfg(not(alloc_frugal))]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    control_backend::get_control_backend().queue_resize_trigger(window_id, width, height)
}
/// Reports a container resize in an allocation-frugal (`mini`) build.
///
/// The signature is identical to the full build's (rule #21: a profile change must
/// not change the API). `mini` runs without a platform singleton, so there is no
/// window whose resize could be observed and nothing to queue — answering `false`
/// reports that honestly instead of accepting a resize that could never be delivered.
#[cfg(alloc_frugal)]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    let _ = (window_id, width, height);
    false
}
/// The client size last reported for `window_id`, or `None` when none was.
///
/// The readback half of [`queue_resize_trigger`]: a host that receives a `Resized`
/// event learns *which* window changed, and asks here *how big* it now is. Falls back
/// to the window's current geometry, which is what it still has if the user has never
/// resized it.
#[cfg(not(alloc_frugal))]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    control_backend::get_control_backend().window_client_size(window_id)
}
/// The last reported client size in an allocation-frugal (`mini`) build.
///
/// Always `None`: the same signature as the full build, but a `mini` build keeps no
/// window records, so no size was ever reported to answer with.
#[cfg(alloc_frugal)]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let _ = window_id;
    None
}
// Clipboard
/// Replaces the platform clipboard's text contents.
///
/// Returns `false` when the clipboard could not be written — including when
/// another application holds it, which is a normal, transient condition. This
/// function takes only text; see [`platform_clipboard`] for the rich backend.
#[cfg(not(alloc_frugal))]
pub fn set_clipboard_text(text: &str) -> bool {
    platform::get_platform().set_clipboard_text(text)
}
/// The platform clipboard's text contents.
///
/// Returns an empty string when the clipboard holds no text, holds a non-text
/// format, or cannot be read — the three cases are **not** distinguished here,
/// so an empty result does not mean the copy succeeded with empty text.
#[cfg(not(alloc_frugal))]
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// The platform clipboard's text, or `None` when it holds no text.
///
/// Where [`get_clipboard_text`] answers `""` for an empty clipboard, a clipboard holding an
/// image or a file list, *and* a failed read, this reports the one fact a paste handler needs:
/// `Some(text)` when there is text to paste, `None` when there is not. It does not by itself
/// distinguish "empty" from "non-text" — use [`platform_clipboard`] for that — but it is the
/// difference between "paste this" and "there is nothing text-shaped to paste".
#[cfg(not(alloc_frugal))]
pub fn clipboard_text() -> Option<String> {
    platform::get_platform().clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
/// Asks the user for a path, using the picker the host's own desktop provides.
///
/// # Why this is a host-level call and not a dialog widget
///
/// [`widget::dialog::file_dialog`] is painted by this library and filled by the **host**: it has no
/// directory listing, so a host without one of its own can browse nothing with it. This call goes
/// to the operating system's picker instead, which browses the filesystem itself.
///
/// # What the three answers mean
///
/// `Ok(Some(path))` the user chose that path; `Ok(None)` the user cancelled (a complete
/// interaction, not a failure); `Err(`[`platform::PathPickerError::Unavailable`]`)` this backend has
/// a picker but cannot present it here (no display session, or a thread the toolkit forbids a modal
/// window on); `Err(`[`platform::PathPickerError::Unsupported`]`)` this backend has none, which
/// [`platform::capabilities`]'.`file_dialog` reports **before** the call so a host can pick its
/// fallback without provoking the failure.
///
/// # This blocks
///
/// The picker is modal: it owns the user's input until it is answered, and the event loop keeps
/// running while it is up. Call it from the thread that owns the loop.
#[cfg(not(alloc_frugal))]
pub fn choose_path(
    purpose: crate::platform::PathPurpose,
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    platform::get_platform().choose_path(purpose, title)
}
/// Runs `work` on the event loop's own thread, at or after `after` from now.
///
/// # What a host uses this for
///
/// A host that is not driving the loop sometimes has to get work *onto* it: ending a session after
/// a deadline (a smoke test that must terminate itself, without a watcher thread and without a
/// process kill), refreshing a status line, polling something the toolkit wants read on its own
/// thread. Every toolkit has a timer API for that, and none of them may be created from another
/// thread — which is exactly where such a host is standing.
///
/// # The answer, and its precision
///
/// Returns whether a loop in this process will run the work: `true` when a loop has ticked, so it
/// reaches the queue on its next frame. `false` means no loop has ticked yet; the work stays
/// queued, and a loop started afterwards runs it — so "schedule the ending, then run the loop"
/// works from a single thread.
///
/// `after` is a **lower bound**: work runs on the first frame at or after its deadline, so the
/// granularity is one frame interval (~16 ms). Repeating work is done by having the callback call
/// this again; there is deliberately no repeating form (one mechanism, one interval).
///
/// See `platform::main_thread` for why the frame tick is the
/// executor and what a panicking callback does.
#[cfg(not(alloc_frugal))]
pub fn schedule_on_main(after: std::time::Duration, work: impl FnOnce() + Send + 'static) -> bool {
    platform::main_thread::schedule_in(after, work)
}
/// Asks the user to pick an existing file — [`choose_path`] with
/// [`PathPurpose::OpenFile`](crate::platform::PathPurpose::OpenFile).
///
/// The three named entry points below are the vocabulary a host asks in; they are one call with
/// one implementation, so they cannot drift apart (principles #54/#101).
#[cfg(not(alloc_frugal))]
pub fn open_path(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::OpenFile, title)
}
/// Asks the user to name a path to write — [`choose_path`] with
/// [`PathPurpose::SaveFile`](crate::platform::PathPurpose::SaveFile).
#[cfg(not(alloc_frugal))]
pub fn save_path(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::SaveFile, title)
}
/// Asks the user to pick a directory — [`choose_path`] with
/// [`PathPurpose::SelectDirectory`](crate::platform::PathPurpose::SelectDirectory).
#[cfg(not(alloc_frugal))]
pub fn select_directory(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::SelectDirectory, title)
}
// Menu operations
/// Creates a native menu bar as a child of `parent`.
///
/// On macOS the bar only becomes the application's main menu once it is attached
/// with [`attach_menu_bar_to_window`]. Add menus with [`create_menu`] and items
/// with [`menu_add_item`].
#[cfg(not(alloc_frugal))]
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MenuBar).create_menu_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Adds a menu under a menu bar (or another menu, for a submenu).
///
/// `parent` must be the **menu bar** for a top-level menu, or a **menu** for a nested one:
/// the parent determines both where the menu belongs structurally and, for a bar, the title
/// that gets painted in the strip. A menu whose parent is a bar also appears in the bar's
/// entry list; a context menu is created under the window it belongs to and starts hidden.
///
/// The menu is a popup and starts hidden. Open it with [`menu_open_at`] (or
/// [`MenuHandle::open_at`](crate::app::MenuHandle::open_at)) when the host decides it should
/// appear — including from the bar's
/// [`triggered_entry`](crate::MenuBar::triggered_entry) event, whose payload carries the
/// entry's rectangle.
///
/// Returns `0` on a profile that does not ship menus.
#[cfg(not(alloc_frugal))]
pub fn create_menu(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::Menu).create_menu(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Attaches a menu bar to a window.
///
/// Returns `false` when the backend cannot attach it.
#[cfg(not(alloc_frugal))]
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    control_backend::get_control_backend().attach_menu_bar_to_window(window, menu_bar)
}
/// Adds an item to a menu, returning the item's id.
///
/// `shortcut` is **display text** and is neither parsed nor registered as an
/// accelerator — use [`format_shortcut`] to render a platform-correct string
/// from a typed shortcut declaration. The returned id is what
/// [`poll_menu_triggered`] reports once the item is chosen.
#[cfg(not(alloc_frugal))]
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    control_backend::get_control_backend().menu_add_item(parent_menu, text, shortcut)
}
/// Whether the running platform can put an icon on a **native** menu item.
///
/// Ask this before building an icon for a native menu: `false` means the OS surface has no place for
/// one, so the caller keeps its text instead of dropping an icon it already resolved. That
/// distinction — *this surface cannot present an icon* versus *the icon failed to load* — is what
/// makes the two reportable apart (blue27 §19.11, §19.13 I15).
///
/// The self-drawn menus (`Menu`, `MenuBar`, `MenuButton`, `DropdownMenu`) do not need this: they are
/// widgets and take a typed source through their own `icon_source` property. This answers only for
/// the native menu bar the platform owns.
///
/// `false` on every desktop backend today; see
/// [`Platform::supports_menu_item_icons`](crate::platform::Platform::supports_menu_item_icons) for
/// what each platform would have to verify before answering `true`.
#[cfg(not(alloc_frugal))]
pub fn menu_item_icons_supported() -> bool {
    control_backend::get_control_backend().supports_menu_item_icons()
}

/// Legacy mechanism-vocabulary spelling of [`menu_item_icons_supported`].
#[cfg(not(alloc_frugal))]
#[deprecated(
    note = "use `menu_item_icons_supported`; the question is about menu items, not a mechanism"
)]
pub fn native_menu_item_icons_supported() -> bool {
    menu_item_icons_supported()
}
/// Renders a shortcut the way the current operating system writes it.
///
/// macOS returns `⌘⇧Z`-style glyphs; Windows and Linux return
/// `Ctrl+Shift+Z`. Use this for any label the user reads (menu text, tooltips,
/// the shortcut column of a command palette) so one shortcut declaration reads
/// natively on every platform.
///
/// For the host-independent notation (useful in serialized keymaps and tests),
/// use [`crate::shortcut::Shortcut::format_shortcut`] together with
/// [`crate::shortcut::format_shortcut_for_platform`] instead.
///
/// ```
/// use rust_widgets::shortcut::{Key, Shortcut};
///
/// let shown = rust_widgets::format_shortcut(&Shortcut::primary(Key::Z));
/// #[cfg(target_os = "macos")]
/// assert_eq!(shown, "⌘Z");
/// #[cfg(not(target_os = "macos"))]
/// assert_eq!(shown, "Ctrl+Z");
/// ```
#[cfg(not(alloc_frugal))]
pub fn format_shortcut(shortcut: &crate::shortcut::Shortcut) -> String {
    platform::get_platform().format_shortcut(shortcut)
}
/// Takes the id of the next menu item the user activated, or `None` when none is
/// queued.
///
/// Like the widget version this drains a queue, so a loop calling it until
/// `None` collects every pending activation. The id returned is the one
/// [`menu_add_item`] handed back.
#[cfg(not(alloc_frugal))]
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_menu_triggered()
}
/// Returns the accelerator text bound to a menu item, if any.
///
/// Read from the menu item's own `shortcut` property, so the answer describes the
/// item the library drew rather than a host-side copy that could drift.
#[cfg(not(alloc_frugal))]
pub fn menu_item_shortcut(menu_item: crate::core::ObjectId) -> Option<String> {
    control_backend::get_control_backend().menu_item_shortcut(menu_item)
}

/// Shows a menu popup with its top-left corner at `position`.
///
/// `position` is in the **window's client coordinates** — the space a menu bar's
/// [`MenuBar::entry_rect`](crate::MenuBar::entry_rect) reports — and the popup is clamped
/// into `viewport` so it never opens partly off-screen. Opening also moves the menu to the
/// end of its window's child list, so a popup paints above, and receives clicks before, any
/// control created after it.
///
/// This is the self-drawn counterpart of a native `NSMenu`/`HMENU` popup: without it a
/// [`create_menu`] widget starts hidden (a popup is not a panel) and has no public way to
/// become visible. Use [`menu_close`] to dismiss it.
///
/// Returns `false` when `menu` is not a mounted menu, when the viewport is empty, or when
/// the id addresses another kind of widget.
///
/// ```rust,no_run
/// use rust_widgets::core::{Point, Rect};
///
/// let window = rust_widgets::create_window("Menu", 0, 0, 640, 480);
/// let bar = rust_widgets::create_menu_bar(window, 0, 0, 640, 28);
/// let file = rust_widgets::create_menu(bar, "File", 0, 0, 160, 0);
/// rust_widgets::menu_add_item(file, "Open", Some("Ctrl+O"));
/// assert!(rust_widgets::menu_open_at(file, 0, 28, Rect::new(0, 0, 640, 480)));
/// assert!(rust_widgets::menu_close(file));
/// ```
#[cfg(full_widgets)]
pub fn menu_open_at(
    menu: crate::core::ObjectId,
    x: i32,
    y: i32,
    viewport: crate::core::Rect,
) -> bool {
    if viewport.width == 0 || viewport.height == 0 {
        return false;
    }
    crate::widget::runtime::open_context_menu(menu, crate::core::Point::new(x, y), viewport)
}

/// Hides an open menu popup.
///
/// Returns `false` when `menu` is not a mounted menu or another kind of widget. The
/// menu's own `hidden`/`about_to_hide` signals fire through [`crate::Menu`]'s close path,
/// so a caller that subscribed to them sees the same lifecycle fact a click-outside would
/// produce.
#[cfg(full_widgets)]
pub fn menu_close(menu: crate::core::ObjectId) -> bool {
    let hidden = crate::widget::runtime::with_widget_mut(menu, |widget| {
        let Some(menu) = (widget as &mut dyn std::any::Any).downcast_mut::<crate::Menu>() else {
            return false;
        };
        menu.hide();
        true
    })
    .unwrap_or(false);
    if hidden {
        crate::widget::runtime::request_repaint_subtree(menu);
        crate::widget::runtime::clear_popup_grab();
    }
    hidden
}
/// Returns the backend's opaque handle for a widget, when it has one.
///
/// Named for the **intent** ("the backend's own handle for this control"), not for
/// the mechanism. The value is opaque: it is whatever object the backend uses to
/// represent the control, and its meaning is entirely backend-specific. It exists so
/// host code and integration tests can reach the underlying object for things the
/// cross-platform API does not model.
///
/// This was `native_handle`, which named a mechanism in the public API — the same
/// vocabulary the surface API deliberately avoids (`src/lib.rs`'s mount/resize
/// helpers, and principle #52). The old name remains as a deprecated alias so an
/// existing call site gets a warning rather than a build break.
///
/// Returns `None` when the widget is unknown, or when the backend created it in
/// state-only mode (for example off the UI thread) and therefore holds no object to
/// return.
#[cfg(not(alloc_frugal))]
pub fn backend_handle(widget: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().get_native_handle(widget)
}
/// Queues activation of a menu item as if the user had chosen it, for tests.
///
/// Returns `false` when the backend will not accept the injected event.
#[cfg(not(alloc_frugal))]
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
/// Creates a tool bar strip as a child of `parent`.
///
/// The returned id can be wrapped in an `app::ToolBarHandle` for the
/// typed operations (adding actions, changing orientation).
///
/// Returns `0` on a profile that does not ship the tool bar.
#[cfg(not(alloc_frugal))]
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ToolBar).create_tool_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a status bar as a child of `parent`, with `text` as its message.
///
/// Returns `0` on a profile that does not ship the status bar.
#[cfg(not(alloc_frugal))]
pub fn create_status_bar(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::StatusBar)
            .create_status_bar(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
// Drag and Drop
/// Starts a drag of `payload` out of `source_widget_id`, advertising it as
/// `mime`.
///
/// `payload` is copied into the platform's drag object and is not retained by
/// this library, so the caller may free it once this returns. Returns `false`
/// when the backend cannot begin a drag — for instance when a drag is already in
/// progress — in which case no drop event will follow.
#[cfg(not(alloc_frugal))]
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
/// Takes the next completed drop, or `None` when none is pending.
///
/// Polling a queue: each successful call removes one event. A drop is only
/// reported after the user has released, so a drag in progress yields `None`.
#[cfg(not(alloc_frugal))]
pub fn poll_drop_event() -> Option<DropEvent> {
    platform::get_platform().poll_drop_event()
}
/// Queues a drop event as if the user had performed it, for tests.
///
/// Returns `false` when the backend will not accept the injected event, in which
/// case a later poll will not report it.
#[cfg(not(alloc_frugal))]
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
/// Enables or disables input-method (IME) input for a text-entry widget.
///
/// Returns `false` when the backend refuses — typically because the id is not a
/// text-entry control, or because the platform has no IME support to switch.
#[cfg(not(alloc_frugal))]
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    control_backend::get_control_backend().set_widget_ime_enabled(widget_id, enabled)
}
/// Whether input-method input is enabled for a widget.
///
/// Reports `false` both for "enabled is off" and for "this id is unknown or not
/// a text control", so it cannot distinguish the two.
#[cfg(not(alloc_frugal))]
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
/// Sets the accessible name of a widget, as reported to assistive technology.
///
/// This is the label a screen reader announces — distinct from the visible text,
/// which is often too terse or ambiguous to read aloud. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    control_backend::get_control_backend().set_widget_accessibility_name(widget_id, name)
}
/// The accessible name set by [`set_widget_accessibility_name`].
///
/// Returns an empty string when no name was set, when the id is unknown, or when
/// the backend refused — the cases are not distinguished, so an empty result
/// does not prove the widget is nameless to assistive technology.
#[cfg(not(alloc_frugal))]
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_accessibility_name(widget_id)
}
// Re-exports from platform module for convenience
#[allow(deprecated)]
pub use platform::NativeCapabilityContract;
#[cfg(not(alloc_frugal))]
pub use platform::{
    backend_name, capabilities, dpi_scale_factor, get_platform, init as platform_init,
    quit as platform_quit, run as platform_run, runtime_gui_mode, runtime_gui_mode_for,
};
pub use platform::{
    CapabilityContract, DropEvent, EmbeddedCapabilityContract, HostedCapabilityContract,
    MobileBackend, OpenError, PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent,
    WidgetTriggerKind, WindowStateFlag,
};

// ═══════════════════════════════════════════════════════
// Deprecated aliases kept for API compatibility (principle #21)
// ═══════════════════════════════════════════════════════

/// Deprecated spellings of the surface API.
///
/// These names said "custom", which described a rendering mechanism — "painted
/// by the library rather than mapped onto an OS control". That distinction no
/// longer exists: the host platform supplies only a window and a drawing surface
/// and the library paints every widget, so the mechanism vocabulary has no
/// referent. Use the `*_surface` names instead.
///
/// The forwards are re-exported at the crate root below, so an existing
/// `rust_widgets::mount_custom_widget(...)` call keeps compiling and only gains a
/// deprecation warning — the caller is told the name changed rather than being
/// broken by it (principle #21).
///
/// # Capability vocabulary (H-318/H-319)
///
/// "Native" described an implementation mechanism, not the capability split the type
/// carries. The canonical spellings are `HostedCapabilityContract`,
/// `CapabilityContract::Hosted` and `Platform::hosted_capability_contract`; the type alias
/// and trait method keep a deprecated forward for one release. The enum variant itself was
/// renamed (an enum variant cannot be aliased), so `CapabilityContract::Native(..)` becomes
/// `CapabilityContract::Hosted(..)`. `native_menu_item_icons_supported()` is now
/// [`menu_item_icons_supported()`], with the old name forwarded and deprecated.
#[allow(deprecated)]
/// # Reachability
///
/// **State:** Reserved: the compatibility shim for APIs replaced in earlier
/// rounds; it exists so old call sites keep compiling. Removal condition: on the
/// next major version.
///
/// Deprecated aliases for APIs renamed in earlier rounds.
///
/// Every function here forwards to its replacement and carries a `#[deprecated]`
/// note naming that replacement, so a caller gets a compiler warning that says
/// what to use instead.
pub mod deprecated {
    /// Deprecated alias of [`crate::mount_surface`].
    #[deprecated(
        note = "renamed to `mount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn mount_custom_widget(
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        crate::mount_surface(parent, id, rect)
    }

    /// Deprecated alias of [`crate::resize_surface`].
    #[deprecated(
        note = "renamed to `resize_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn resize_custom_widget(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        crate::resize_surface(id, rect)
    }

    /// Deprecated alias of [`crate::unmount_surface`].
    #[deprecated(
        note = "renamed to `unmount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn unmount_custom_widget(id: crate::core::ObjectId) -> bool {
        crate::unmount_surface(id)
    }

    /// Deprecated alias of [`crate::invalidate_surface`].
    #[deprecated(
        note = "renamed to `invalidate_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn request_custom_repaint(id: crate::core::ObjectId) -> bool {
        crate::invalidate_surface(id)
    }

    /// Deprecated alias of [`crate::supports_surfaces`].
    #[deprecated(
        note = "renamed to `supports_surfaces`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn supports_custom_widgets() -> bool {
        crate::supports_surfaces()
    }

    /// Deprecated alias of [`crate::backend_handle`].
    ///
    /// Gated with its replacement: the `mini` profile has no platform singleton to ask,
    /// so `backend_handle` does not exist there and an ungated alias would name a
    /// missing function.
    #[cfg(not(alloc_frugal))]
    #[deprecated(
        note = "renamed to `backend_handle`; 'native' named a mechanism, and which object a \
                backend holds is its own business (principle #52)"
    )]
    pub fn native_handle(widget: crate::core::ObjectId) -> Option<usize> {
        crate::backend_handle(widget)
    }
}

// Old crate-root paths keep resolving, so a rename is a warning rather than a
// break. Exported here rather than relying on callers reaching into `deprecated`.
#[allow(deprecated)]
pub use deprecated::{
    mount_custom_widget, request_custom_repaint, resize_custom_widget, supports_custom_widgets,
    unmount_custom_widget,
};
// Gated with `backend_handle`, which the alloc-frugal profile does not compile (it has
// no platform singleton to ask), so the alias must not be re-exported there.
#[cfg(not(alloc_frugal))]
#[allow(deprecated)]
pub use deprecated::native_handle;

#[cfg(test)]
mod docs_paths_tests;

#[cfg(test)]
mod compat_path_tests {
    /// The old crate-root spellings must keep resolving after the rename.
    ///
    /// The rename is a *deprecation*, not a break (principle #21): existing callers
    /// must still compile. Referring to the functions here fails the build if a
    /// re-export is ever dropped, which is otherwise invisible because nothing else
    /// in the crate uses the old names.
    #[test]
    #[allow(deprecated)]
    fn old_surface_names_still_resolve_at_the_crate_root() {
        let _: fn(crate::core::ObjectId, crate::core::ObjectId, crate::core::Rect) -> bool =
            crate::mount_custom_widget;
        let _: fn(crate::core::ObjectId, crate::core::Rect) -> bool = crate::resize_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::unmount_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::request_custom_repaint;
        let _: fn() -> bool = crate::supports_custom_widgets;
    }

    /// The deprecated forwards must reach the same function, not a stale copy.
    #[test]
    #[allow(deprecated)]
    fn deprecated_aliases_delegate_to_the_surface_api() {
        assert_eq!(crate::supports_custom_widgets(), crate::supports_surfaces());
    }
}
