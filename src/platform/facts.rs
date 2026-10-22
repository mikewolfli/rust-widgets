// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The platform-facts accessor, kept out of `mod.rs` (principle #8).

/// Every build's platform-facts accessor, including `mini`.
///
/// Upper layers (print, GPU adaptation, menu hardware detection) need to ask the
/// backend about OS facts — total memory, battery state, spooler availability.
/// The `mini` profile is deliberately alloc-frugal and has **no platform
/// singleton** (`get_platform` is `not(alloc_frugal)`), so a direct call would
/// fail to compile there.
///
/// Both arms return the *same kind* of value: a host that reports, for every
/// capability it does not have, the honest absence (`None` / `false` / empty)
/// rather than a made-up figure (principle #37). Outside `mini` that is whichever
/// OS backend this target has; inside `mini` it is the portable host. Call sites
/// stay free of `cfg` branching (principle #35).
pub fn platform_facts() -> &'static dyn crate::platform::Platform {
    #[cfg(not(alloc_frugal))]
    {
        crate::platform::runtime::get_platform()
    }
    #[cfg(alloc_frugal)]
    {
        crate::platform::portable::instance()
    }
}
