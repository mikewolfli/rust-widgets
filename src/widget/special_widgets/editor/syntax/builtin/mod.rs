// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `BuiltinHighlighter` facade.
//!
//! `builtin-languages` decides which implementation is compiled: the real
//! dependency-free lexer, or a stub that produces no tokens. The public shape is
//! identical either way, so callers never branch on the feature (BLUE31 §1.5).

#[cfg(feature = "builtin-languages")]
mod native;
#[cfg(feature = "builtin-languages")]
pub use native::BuiltinHighlighter;

#[cfg(not(feature = "builtin-languages"))]
mod stub;
#[cfg(not(feature = "builtin-languages"))]
pub use stub::BuiltinHighlighter;
