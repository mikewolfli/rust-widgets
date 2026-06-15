#!/bin/bash
# ============================================================
# rust_widgets — macOS system dependencies installer
# ============================================================
# This script installs system libraries required by
# rust_widgets features on macOS using Homebrew.
#
# Usage: bash scripts/install-deps-macos.sh
# ============================================================

set -euo pipefail

echo "🔧 Installing rust_widgets system dependencies (macOS)..."
echo ""

# Check for Homebrew
if ! command -v brew &>/dev/null; then
    echo "❌ Homebrew not found. Install from https://brew.sh"
    echo "   Then re-run this script."
    exit 1
fi

# Video codecs (ffmpeg-next → FFmpeg)
echo "📦 [video-codecs] FFmpeg (for video decoding)..."
echo "    brew install ffmpeg"
brew install ffmpeg

echo ""
echo "✅ All system dependencies installed!"
echo ""
echo "Note: audio-output uses CoreAudio (built-in, no extra deps)."
echo "      webkit-engine is Linux-only (simulated on macOS)."
echo ""
echo "Now you can build with:"
echo "  cargo build"
