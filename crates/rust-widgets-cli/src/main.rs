// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `rwui` — the rust_widgets designer CLI.
//!
//! `main` is an entry stub only: argument handling, the commands and their diagnostics live in
//! [`app`] (BLUE31 §2.3).

mod app;
mod bundle;
mod cli;
mod diagnostics;
mod forward;

fn main() -> std::process::ExitCode {
    app::run()
}
