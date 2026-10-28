// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// The payload and its provenance record live in the `rust-widgets-font` crate
// (`crates/rust-widgets-font/src/emoji.rs` in this repository). They moved there when this package
// reached crates.io's 10 MiB ceiling: the payloads are already-compressed binaries, so keeping
// them beside the code meant the code and the data competed for one budget. Cargo excludes the
// nested package (`"!crates/**/*"` in this manifest), and `fonts-*` features pull it back in as a
// dependency.
//
// This module keeps the path and the item every caller already uses.
pub use rust_widgets_font::emoji::FONT;
