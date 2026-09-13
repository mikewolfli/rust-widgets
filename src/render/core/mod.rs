// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core rendering data types and commands.
pub(crate) mod command;
pub(crate) mod types;

pub use command::{BlendMode, RenderCommand};
pub use types::{ShapedText, TextCluster, TextMetrics};
