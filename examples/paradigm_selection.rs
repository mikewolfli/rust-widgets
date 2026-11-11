// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One application, one paradigm decision, made from runtime facts.
//!
//! # What this example is for
//!
//! The library ships two composition paradigms — `AdaptiveScaffold` for touch layouts and
//! `MdiArea` / `SdiWorkbench` for desktop workbenches — and the demos show each one in isolation.
//! What was missing (issue-20261111 R-08) is the decision itself: a single code path that asks
//! the host what it is running on and builds the matching paradigm, so a new application has
//! something to copy.
//!
//! # How the decision is made
//!
//! By the **primary display's width**, a runtime fact. It is deliberately *not* an operating
//! system check: a wide window on a phone in landscape and a narrow window on a desktop both get
//! the paradigm that fits the space, not the one that matches `cfg(target_os)` (principle #35).
//! The breakpoint is the same 840 logical pixels `AdaptiveScaffold` uses internally for its
//! drawer / rail / bottom-bar switch, so the outer choice and the scaffold's own adaptation agree.
//!
//! # Running it
//!
//! ```text
//! cargo run --example paradigm_selection --features desktop
//! ```

use rust_widgets::app::{App, AppConfig};
use rust_widgets::core::Rect;
use rust_widgets::widget::base_widgets::label::Label;
// The crate-root spellings, so the re-export wiring is exercised by this example's build.
use rust_widgets::{AdaptiveScaffoldBuilder, SdiWorkbench};

/// The composition paradigm a host should build for its current viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paradigm {
    /// Bottom navigation + optional drawer: phones and small tablets.
    TouchScaffold,
    /// Independent top-level windows with cascade/tile: desktop workbenches.
    DesktopSdi,
}

/// The width below which the touch paradigm wins.
///
/// Matches `AdaptiveScaffold`'s own drawer/rail breakpoint, so the two decisions cannot disagree
/// about what "wide" means.
const TOUCH_BREAKPOINT: u32 = 840;

/// Chooses the paradigm from the available client width.
///
/// Split out from [`main`] so the decision is testable without a display: it is one comparison
/// and one enum, not a property of the process.
fn paradigm_for(width: u32) -> Paradigm {
    if width < TOUCH_BREAKPOINT {
        Paradigm::TouchScaffold
    } else {
        Paradigm::DesktopSdi
    }
}

/// The primary display's logical width, or the touch-sized default when no display answers.
///
/// A backend that cannot enumerate displays is a headless or state-only one; answering with the
/// smaller layout is the safe direction because a constrained window can still show it.
fn viewport_width() -> u32 {
    let displays = rust_widgets::platform::displays();
    displays
        .iter()
        .find(|display| display.is_primary)
        .or_else(|| displays.first())
        .map(|display| display.bounds.width)
        .unwrap_or(420)
}

fn main() {
    let mut app = App::with_config(
        AppConfig::default().with_app_name("Paradigm Selection").with_organization("rust_widgets"),
    );
    app.init();

    let width = viewport_width();
    match paradigm_for(width) {
        Paradigm::TouchScaffold => build_touch(&mut app, width.min(TOUCH_BREAKPOINT - 1)),
        Paradigm::DesktopSdi => build_desktop(&mut app, width),
    }

    app.run();
}

/// Builds the touch/tablet layout: one window hosting an `AdaptiveScaffold`.
fn build_touch(app: &mut App, width: u32) {
    let height = 780;
    let window = app.new_window("Paradigm — touch scaffold", 40, 40, width.max(320), height);
    let geometry = Rect::new(0, 0, width.max(320), height);

    // The builder is the documented construction path (R-10): it assembles through the same
    // `new()` + setters every hand-written scaffold uses, so the two cannot drift.
    let scaffold = AdaptiveScaffoldBuilder::new("Home", geometry)
        .show_bottom_nav(true)
        .destination("home", "Home")
        .destination("search", "Search")
        .destination("settings", "Settings")
        .body(Box::new(Label::new(
            "Touch paradigm: the scaffold owns the AppBar, the bottom navigation bar and \
             the body slot."
                .to_string(),
            Rect::new(16, 16, geometry.width.saturating_sub(32), 48),
        )))
        .build();

    match window.mount_surface(Box::new(scaffold), geometry) {
        Ok(surface) => println!("touch scaffold mounted: id={:?}", surface.raw_id()),
        Err(error) => eprintln!("touch scaffold could not be mounted: {error}"),
    }
}

/// Builds the desktop layout: an SDI workbench with two independent document windows.
fn build_desktop(app: &mut App, width: u32) {
    // The workbench arranges top-level windows; it does not create a second creation mechanism.
    // `SdiWorkbench::open_window` goes through the same `create_window` entry `App::new_window`
    // uses, and `cascade`/`tile` only call `WindowHandle::set_geometry`.
    let mut workbench = SdiWorkbench::new(Rect::new(0, 0, width, 900));
    let _first = workbench.open_window("Document A", 60, 60, 640, 480);
    let _second = workbench.open_window("Document B", 120, 120, 640, 480);
    let arranged = workbench.cascade();

    // One of the windows still needs a visible body; the second is the proof that the workbench
    // owns independent top-level windows rather than panes inside one of them.
    let body = app.new_window("Paradigm — desktop SDI", 60, 60, 640, 480);
    let _ = body.mount_surface(
        Box::new(Label::new(
            "Desktop paradigm: the host owns each top-level window; the workbench only arranges \
             them."
                .to_string(),
            Rect::new(16, 16, 600, 48),
        )),
        Rect::new(0, 0, 640, 480),
    );

    println!(
        "desktop workbench: {} windows, {} arranged by cascade",
        workbench.window_count(),
        arranged
    );
    for window in workbench.windows() {
        println!("  window id={:?}", window.raw_id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_breakpoint_matches_the_scaffolds_own_switch() {
        assert_eq!(paradigm_for(TOUCH_BREAKPOINT - 1), Paradigm::TouchScaffold);
        assert_eq!(paradigm_for(TOUCH_BREAKPOINT), Paradigm::DesktopSdi);
        assert_eq!(TOUCH_BREAKPOINT, 840, "keep this in step with AdaptiveScaffold's drawer mode");
    }
}
