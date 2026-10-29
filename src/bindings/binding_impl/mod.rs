// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stable C ABI for foreign language bindings.
//!
//! The implementation is split by binding family: the `c_abi*` modules carry the
//! shared C ABI that every foreign-language binding calls, `harmony` is the
//! HarmonyOS/ArkTS native bridge, and `binding_status` holds the per-language
//! version probes. `helpers` and `wire` are shared internals; `tests` holds the
//! inline ABI tests.

mod binding_status;
mod c_abi;
mod c_abi_platform;
mod c_abi_surfaces;
mod c_abi_widgets;
mod harmony;
mod helpers;
mod wire;

#[cfg(test)]
mod tests;

pub use binding_status::*;
pub use c_abi::*;
pub use c_abi_platform::*;
pub use c_abi_surfaces::*;
pub use c_abi_widgets::*;
pub use harmony::*;
pub(crate) use helpers::*;
pub(crate) use wire::*;
