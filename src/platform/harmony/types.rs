// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Harmony desktop backend shell.
use crate::platform::state::BackendState;
use std::fmt;
use std::sync::atomic::AtomicBool;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HarmonyHandleKind {
    Window,
}
pub(crate) struct HarmonyRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}
impl HarmonyRuntimeState {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}
/// Harmony backend platform adapter.
///
/// The backend is state-only: every `WidgetKind` is painted by `src/widget/`, so
/// the state model needs no per-kind handle classification and no side tables for
/// native control payloads.
pub struct HarmonyPlatform {
    pub(crate) state: BackendState<HarmonyHandleKind>,
    pub(crate) runtime: HarmonyRuntimeState,
}
impl fmt::Debug for HarmonyPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarmonyPlatform").finish_non_exhaustive()
    }
}
impl HarmonyPlatform {
    /// Creates a new Harmony platform adapter.
    pub fn new() -> Self {
        Self { state: BackendState::new(), runtime: HarmonyRuntimeState::new() }
    }
}
crate::impl_default_via_new!(HarmonyPlatform);
impl HarmonyPlatform {
    /// Insert widget state and return allocated logical id.
    pub(crate) fn insert_widget(
        &self,
        kind: HarmonyHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
}
