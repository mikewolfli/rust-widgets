// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Win32 helper functions for native control creation.
//!
//! The library is fully self-drawing: every `WidgetKind` is painted by
//! `src/widget/` and `control_backend::routing` returns
//! `ControlRoutePreference::CustomRequired` for every kind, so no `create_*`
//! trait method calls these helpers any more. The module is retained (empty of
//! constructors) as the seam where per-kind Win32 control construction would
//! live again if a kind is ever promoted back to `NativePreferred`.
