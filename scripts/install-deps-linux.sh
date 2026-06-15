#!/bin/bash
# ============================================================
# rust_widgets — Linux system dependencies installer
# ============================================================
# This script installs ALL system libraries required by
# rust_widgets features on Debian/Ubuntu Linux.
#
# Usage: bash scripts/install-deps-linux.sh
#
# For other distros (Fedora, Arch, etc.), adapt the package
# names below to your package manager.
# ============================================================

set -euo pipefail

echo "🔧 Installing rust_widgets system dependencies (Linux)..."
echo ""

# Core GUI/rendering dependencies
echo "📦 [core] GTK3 development headers (required by webkit-engine)..."
sudo apt-get install -y libgtk-3-dev

# Audio output (cpal → ALSA)
echo ""
echo "📦 [audio-output] ALSA development headers (required by cpal)..."
sudo apt-get install -y libasound2-dev

# Web engine (webkit2gtk)
echo ""
echo "📦 [webkit-engine] WebKitGTK + JavaScriptCore + libsoup..."
sudo apt-get install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev

# Video codecs (ffmpeg-next → FFmpeg)
echo ""
echo "📦 [video-codecs] FFmpeg development libraries (all sub-libraries)..."
sudo apt-get install -y \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libavfilter-dev \
    libavdevice-dev \
    libswscale-dev \
    libswresample-dev \
    libpostproc-dev

# LLVM/Clang (required by ffmpeg-sys-next's bindgen)
echo ""
echo "📦 [build] LLVM/Clang development headers (required by bindgen)..."
sudo apt-get install -y llvm-18-dev libclang-18-dev

echo ""
echo "✅ All system dependencies installed!"
echo ""
echo "Now you can build with:"
echo "  cargo build"
echo ""
echo "Or with specific features:"
echo "  cargo build --features full"
