// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

pub(crate) mod create_widgets;
pub mod custom_paint;

pub use custom_paint::CustomPaintControlBackend;

#[cfg(all(test, not(feature = "embedded")))]
mod tests;
