// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform abstraction types and capability contracts.
//!
//! # Why this is a façade
//!
//! One file held every value type the platform layer exchanges **and** the two backend traits —
//! about 3 100 lines, with the trait alone accounting for roughly 2 300 of them. Reading it meant
//! scrolling past two hundred enums to reach `Platform::run`, and editing a `#[derive]` risked
//! touching the contract by accident.
//!
//! The split follows what the file actually contained:
//!
//! * `values.rs` — the data and capability contracts: `Plot2DFamily`-style options, capability and
//!   display records, path and drop events, cursor and orientation enums.
//! * `traits.rs` — [`Platform`] and [`MobilePlatformExtension`], the two things a backend implements.
//!
//! Both are re-exported here, so every existing path (`platform::types::WidgetTriggerKind`,
//! `crate::platform::Platform`, …) keeps working. The trait stays one block rather than being
//! composed out of capability supertraits: every backend implements it whole, and splitting it
//! would rewrite all nine backends' `impl` headers for a change no caller can observe.

mod traits;
mod values;

pub use traits::*;
pub use values::*;

// `FRAME_INTERVAL_MS` is `pub(crate)`, so a `pub use` cannot carry it; the crate-internal path
// (`crate::platform::types::FRAME_INTERVAL_MS`) is what nine backends use.
pub(crate) use values::FRAME_INTERVAL_MS;
