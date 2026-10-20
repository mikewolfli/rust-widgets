#!/bin/sh
# Lets a macOS host type-check the GTK backend for a Linux target.
#
# # Why this is needed
#
# The GTK code (`src/platform/linux/**`) is gated on
# `all(unix_desktop, feature = "gtk-native")`, so on a macOS host `cargo check`
# never sees it. Cross-checking it with `--target x86_64-unknown-linux-gnu` works
# as long as `gtk+3` and `pkg-config` are installed on the host, with two
# obstacles this shim removes:
#
# 1. `pkg-config` refuses cross-compilation unless `PKG_CONFIG_ALLOW_CROSS=1`.
# 2. This host's `gdk-3.0.pc` was generated for macOS and carries Apple
#    `-Wl,-framework,Cocoa` flags. `rustc` rejects `kind="framework"` for a
#    non-Apple target, so they are filtered out of pkg-config's answer.
#
# Only `cargo check`/`cargo clippy` should use this: they never link, so dropping
# the framework flags costs nothing. Do **not** use it for `cargo build`.
#
# Usage:
#
#   export PKG_CONFIG_ALLOW_CROSS=1
#   export PKG_CONFIG="$PWD/tools/pkgconfig_linux_cross_shim.sh"
#   cargo clippy --target x86_64-unknown-linux-gnu \
#     --no-default-features --features "tablet,gtk-native" --lib -- -D warnings
out=$(/opt/homebrew/bin/pkg-config "$@") || exit $?
printf '%s\n' "$out" | sed \
    -e 's/ -Wl,-framework,[A-Za-z0-9_.\/-]*//g' \
    -e 's/ -framework [A-Za-z0-9_.\/-]*//g'
