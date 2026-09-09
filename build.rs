// Build script for rust_widgets.
//
// Detects required system libraries at build time and prints
// helpful installation instructions if they are missing.
//
// Feature-specific system dependencies:
//   audio-output  → libasound2-dev (Linux), CoreAudio (macOS), WASAPI (Windows)
//   webkit-engine → libwebkit2gtk-4.1-dev + deps (Linux-only)
//   video-codecs  → libavcodec-dev, ... (Linux), brew ffmpeg (macOS), vcpkg (Windows)

fn main() {
    check_system_dependencies();
}

fn feature_enabled(name: &str) -> bool {
    let env_name = format!("CARGO_FEATURE_{}", name.to_uppercase().replace('-', "_"));
    std::env::var(env_name).is_ok()
}

fn check_system_dependencies() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "linux" => check_linux(),
        "macos" => check_macos(),
        "windows" => check_windows(),
        _ => {}
    }
}

fn check_linux() {
    if feature_enabled("audio-output") {
        check_pkg("alsa", "libasound2-dev", "sudo apt-get install -y libasound2-dev");
    }
    if feature_enabled("webkit-engine") {
        check_pkg("webkit2gtk-4.1", "libwebkit2gtk-4.1-dev",
            "sudo apt-get install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev");
        check_pkg("gtk+-3.0", "libgtk-3-dev", "sudo apt-get install -y libgtk-3-dev");
    }
    if feature_enabled("video-codecs") {
        check_ffmpeg();
    }
}

fn check_macos() {
    if feature_enabled("video-codecs") {
        check_ffmpeg();
    }
}

fn check_windows() {
    if feature_enabled("video-codecs") {
        warn("'video-codecs' feature requires FFmpeg. Install: vcpkg install ffmpeg");
    }
}

fn check_pkg(name: &str, dev_pkg: &str, install_cmd: &str) {
    let ok = std::process::Command::new("pkg-config")
        .args(["--exists", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        // Keep successful dependency probes silent; Cargo warnings are for
        // actionable missing dependencies only.
    } else {
        warn(&format!("Missing system library: {name} ({dev_pkg})"));
        warn(&format!("  Install: {install_cmd}"));
    }
}

fn check_ffmpeg() {
    let mut libs = vec![
        "libavcodec",
        "libavformat",
        "libavutil",
        "libavfilter",
        "libavdevice",
        "libswscale",
        "libswresample",
        "libpostproc",
    ];
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // Homebrew's FFmpeg formula does not ship libpostproc; ffmpeg-next's
        // video path does not require it.
        libs.retain(|lib| *lib != "libpostproc");
    }
    let mut all_ok = true;
    for lib in &libs {
        let ok = std::process::Command::new("pkg-config")
            .args(["--exists", lib])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            // Keep successful dependency probes silent; missing libraries are
            // reported below with an actionable installation command.
        } else {
            println!("cargo:warning=  ❌ Missing: {lib}");
            all_ok = false;
        }
    }
    if !all_ok {
        warn("Missing FFmpeg development libraries. Install all:");
        warn("  sudo apt-get install -y libavcodec-dev libavformat-dev \\");
        warn("    libavutil-dev libavfilter-dev libavdevice-dev \\");
        warn("    libswscale-dev libswresample-dev libpostproc-dev");
    }
}

fn warn(msg: &str) {
    println!("cargo:warning={msg}");
}
