// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform-specific IME stubs.
//!
//! The `macos` module provides a macOS IME bridge stub that uses
//! `NSTextInputContext` objc2 APIs when the `macos` feature is active.
//!
//! Windows IME is handled by `super::ime_windows::WindowsImeBridge`
//! (the real implementation without fake COM vtable stubs).
