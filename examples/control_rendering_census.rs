// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Control rendering census probe (BLUE20 layer 1) — the human-facing view.
//!
//! Renders **every** control the factory publishes, twice (light and dark), and
//! prints a per-control pixel census. The same measurement is asserted by
//! `tests/control_rendering_census_test.rs`; this binary exists so a person can
//! read the raw table, and so `tools/control_rendering_baseline.txt` has a
//! reproducible producer.
//!
//! # Reading the table
//!
//! | Column | Meaning |
//! |---|---|
//! | `ink(l)` | pixels the control painted, under the light appearance |
//! | `dominant(l)` | the colour it painted most, light |
//! | `dominant(d)` | the colour it painted most, dark |
//! | `P3` | `yes` when those two differ — the control responds to the theme |
//! | `detail` | painted pixels that are not the dominant colour: text, borders |
//!
//! A control with `ink(l)=0` paints nothing at all; one with `P3=NO` renders the
//! same in both appearances, which means its chrome is hardcoded.
//!
//! # Why the factory is walked by name
//!
//! `WidgetKind` has fewer variants than the registry has controls — 13 kinds are
//! shared by 2–5 controls — so a kind sweep would silently skip 19 of them. The
//! census enumerates `WidgetFactory::widget_names`, which cannot under-count.
//!
//! # Gating
//!
//! Requires the full widget set and the theme module, i.e. the `desktop` profile.

#![cfg(all(not(feature = "mini"), not(target_arch = "wasm32")))]

use rust_widgets::theme::theme_test_guard;
use rust_widgets::widget::census::{census_all_controls, format_row, install_preset_appearances};

fn main() {
    // Serialises against any other test or probe that switches the process-wide
    // theme, so a concurrent reader cannot see a half-switched palette.
    let _guard = theme_test_guard();
    install_preset_appearances();

    let rows = census_all_controls();

    println!(
        "{:<28} {:>8} {:>14} {:>14} {:>7} {:>8}",
        "name", "ink(l)", "dominant(l)", "dominant(d)", "P3", "detail"
    );
    for row in &rows {
        println!("{}", format_row(row));
    }

    let invisible = rows.iter().filter(|row| !row.light.paints_anything()).count();
    let theme_blind = rows.iter().filter(|row| !row.differs_between_appearances()).count();
    println!("\nchecked={} invisible={} theme_blind={}", rows.len(), invisible, theme_blind);
}
