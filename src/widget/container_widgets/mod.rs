// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Container widgets: tab widgets, scroll areas, MDI areas, etc.
#[cfg(widgets_unstripped)]
pub mod carousel;
#[cfg(widgets_unstripped)]
pub mod collapsible_pane;
#[cfg(widgets_unstripped)]
pub mod dockwidget;
pub mod groupbox;
#[cfg(widgets_unstripped)]
pub mod masonry_layout;
#[cfg(widgets_unstripped)]
pub mod mdiarea;
#[cfg(widgets_unstripped)]
pub mod safe_area;
pub mod scrollarea;
#[cfg(widgets_unstripped)]
pub mod splitter;
#[cfg(widgets_unstripped)]
pub mod stackedwidget;
#[cfg(widgets_unstripped)]
pub mod stepper;
#[cfg(widgets_unstripped)]
pub mod tabwidget;
#[cfg(widgets_unstripped)]
pub mod toolbox;
// Re-export container widgets
#[cfg(widgets_unstripped)]
pub use carousel::{Carousel, WidgetAndDraw};
#[cfg(widgets_unstripped)]
pub use collapsible_pane::CollapsiblePane;
#[cfg(widgets_unstripped)]
pub use dockwidget::DockWidget;
pub use groupbox::GroupBox;
#[cfg(widgets_unstripped)]
pub use masonry_layout::{MasonryItem, MasonryLayout};
#[cfg(widgets_unstripped)]
pub use mdiarea::MdiArea;
#[cfg(widgets_unstripped)]
pub use safe_area::{SafeArea, SafeAreaInsets};
pub use scrollarea::ScrollArea;
#[cfg(widgets_unstripped)]
pub use splitter::Splitter;
#[cfg(widgets_unstripped)]
pub use stackedwidget::StackedWidget;
#[cfg(widgets_unstripped)]
pub use stepper::Stepper;
#[cfg(widgets_unstripped)]
pub use tabwidget::TabWidget;
#[cfg(widgets_unstripped)]
pub use toolbox::ToolBox;

/// `Toolbox` — the container-side spelling of the `ToolBox` control.
///
/// `WidgetKind::Toolbox` is the kind every toolbox spelling reports, and the
/// capability layer registers `tool_box` / `toolbox` against it. The type keeps its
/// historical `ToolBox` spelling (it predates the kind), and this alias is what makes
/// the two names interchangeable from Rust as well, so a caller who writes the
/// control's name the way the enum does is not left with a type that does not exist.
/// It is a type alias rather than a second struct: two structs would be two controls
/// whose state could diverge (rule #23).
#[cfg(widgets_unstripped)]
pub type Toolbox = ToolBox;
