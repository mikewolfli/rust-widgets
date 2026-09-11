# rust_widgets Future Work

This file tracks items that are currently not feasible to complete in the present workspace/runtime constraints.

## Rules

1. This file uses hierarchical TODO management, listing each top-level ITEM one by one.
2. Unfinished items are left blank; completed items are marked "Completed" with a timestamp (e.g., Completed 2026-03-02).
3. Deleted or canceled items are marked "Canceled" or "Deleted" with a timestamp.
4. After all todo list are finished, review and check if there are gaps, added them after current version todo list.

## Current Future Requirements (v1)

## ITEM List

- [ ] ITEM 0: Hybrid policy true per-control compile-time routing closure
  - Constraint: current hybrid route is native-first for shared/basic control APIs; advanced-control routing matrix is not yet fully generated per control kind.
  - Blocker: requires stable per-control capability table generation + compile-time route validation across all target profiles.
  - Target completion signal: `control-policy-hybrid` can prove deterministic native/basic + custom/advanced routing with parity gates.
  - Status 2026-09-11: largely delivered, but not yet *compile-time*. `tools/generate_control_route_matrix.py` emits a per-`WidgetKind` route matrix and the gate in `tools/check_control_route_matrix.sh` passes `--fail-on-placeholder --fail-on-contract-miss`; measured results are 167 variants with 0 missing route mappings and 0 missing trait create-method contracts. What remains open is the *compile-time* half: the route is currently validated by a generated report + gate rather than enforced by the type system.

- [ ] ITEM 1: Linux full native parity without `gtk-native` feature
  - Constraint: current non-`gtk-native` path is a preview/state loop and does not provide guaranteed visible native widgets.
  - Blocker: requires stable non-GTK native backend strategy (or mandatory GTK runtime dependency policy).
  - Target completion signal: native-interactive behavior parity with `gtk-native` path.
  - Note 2026-09-11: the `gtk-native` path itself is now fully verified — the `image`-dependent `dav1d` system library was built from source into `~/.local`, so `--features desktop` compiles and its 3787 lib tests pass (0 failed / 0 ignored), and the GTK lifecycle test executes against a real X display.

- [x] ITEM 1b: Wayland backend verified against a real compositor
  - Completed 2026-09-11
  - Constraint was: development hosts run an X11 session with no compositor installed and no passwordless sudo, so the native `xdg_toplevel` path was never exercised.
  - Resolution: `apt-get download` (no root) fetches weston 13.0.0 + libweston into `~/.local`; the compiled-in module directory inside libweston is patched in place to a strictly shorter `~/.local` path so the binary layout is preserved; weston then runs headless (`headless-backend` + `kiosk-shell`, which needs no client processes).
  - Reproducer: `tools/run_wayland_compositor_tests.sh` (three modes: `system` for hosts with sudo/a preinstalled weston, `rootless` for unprivileged dev hosts, `auto` to pick).
  - Target completion signal met: with the compositor live, `native_session_binds_live_compositor` asserts the backend bound real `wl_compositor` and `xdg_wm_base` globals (`wl_compositor=yes xdg_wm_base=yes`); the full Wayland suite passes 14/14, and the negative control confirms `native_session` stays empty without a compositor.
  - CI 2026-09-11: the `wayland-compositor` job in `.github/workflows/ci.yml` installs weston + libdav1d-dev and runs the same script in `system` mode, so the real-compositor path is now continuously verified rather than only verified once.

- [ ] ITEM 2: Harmony desktop native window/render/event-loop integration
  - Constraint: current Harmony desktop backend is preview/state-loop oriented.
  - Blocker: missing production Harmony desktop native bridge and event dispatch binding in this repo scope.
  - Progress 2026-09-11: `aarch64-unknown-linux-ohos` cross-compile verified (0 warnings); capability contract made honest (`native_menu: false`) instead of inheriting desktop defaults; module status documented in `src/platform/harmony/status.md`.
  - Remaining blocker: OpenHarmony SDK (ArkUI N-API headers) is not available in this environment (checked: no `~/ohos-sdk`/`~/OpenHarmony`), so no native ArkUI object can be constructed or tested.
  - Target completion signal: real native window lifecycle, input dispatch, and widget host integration.

- [ ] ITEM 2b: Windows native SpinBox / ListView / ScrollArea (implementation landed, runtime unverified)
  - Progress 2026-09-11: the three controls that were state-only surrogates now create real Win32 objects — `msctls_updown32` (with `UDS_SETBUDDYINT`/`UDS_ALIGNRIGHT`/`UDS_ARROWKEYS`/`UDS_WRAP`), `SysListView32` in report view (with an inserted full-width column and `LVS_EX_FULLROWSELECT`), and a `WS_HSCROLL | WS_VSCROLL` child window with an initial scroll range. See `src/platform/windows/helpers.rs`.
  - Verification achieved here: compiles warning-free for `x86_64-pc-windows-msvc` and `x86_64-pc-windows-gnullvm`, clippy `-D warnings` clean, and the Windows-only test module type-checks against the Windows target. The Windows path is now continuously checked by the `windows-cross-check` job in `.github/workflows/ci.yml`.
  - Remaining blocker: real Win32 runtime verification (this host has no Windows and no Wine; `cargo test` for the Windows target also needs an MSVC/mingw C toolchain that is not installed).
  - Target completion signal: the controls render and respond on a real Windows machine.

- [x] ITEM 3: Android mobile runtime lifecycle + view bridge (desktop-run parity excluded)
  - Completed 2026-09-11
  - Delivered: `bindings` FFI gate widened off `desktop`; Rust→Java view factory (`create_native_view`) plus native setters; `MessageBox` as a real `android.app.AlertDialog`; `jni_available()` readiness fix; runtime backend selection (`create_native_platform` Android/Harmony arms + `Platform::mobile_extension`); dependency-free logcat logger; JNI signature gate (`tools/check_jni_signatures.sh`); NDK cross-compile; Gradle-free APK build + emulator runner (`tools/build_android_testapp.sh`, `tools/run_android_testapp.sh`).
  - Target completion signal met: end-to-end emulator run reports `RESULT: PASS` for both Java→Rust and Rust→Java directions, with a native view backing every created control.
  - Physical device 2026-09-11: verified on a Xiaomi M2102J2SC (arm64-v8a, Android 13) — signed arm64 APK installs and runs, `RESULT: PASS`, `primaryCpuAbi=arm64-v8a`, zero `AndroidRuntime` errors.
  - File dialog 2026-09-11: `create_file_dialog` now launches a real `ACTION_OPEN_DOCUMENT` (system documents picker) through the stored Activity recovered by reflection; observed on-device as `Displayed com.android.documentsui/.picker.PickActivity`. `ColorDialog`/`FontDialog` stay logical-only because Android ships no platform picker for either (a platform fact, not a gap).
  - Toolbar 2026-09-11: the "not guaranteed" note was upgraded from speculation to an on-device measured constraint — the AndroidX class loads but cannot be constructed without AAR resource merging (`NoClassDefFoundError: androidx/appcompat/R$attr`; the AAR ships 545 resource files). The library deliberately does not bundle AndroidX.
  - Remaining (tracked separately): AndroidX Toolbar nativisation inside the library; additional device/ABI coverage.
  - Native controls 2026-09-11 (Rounds 2-5): of the 23 widget kinds whose *native* path silently degraded to a different primitive, **19 now have real native objects on Linux (gtk-native) and Windows**. Round 2: GroupBox/Frame/TabWidget/Splitter (gtk::Frame/Notebook/Paned; BS_GROUPBOX/SysTabControl32/client-edge host). Round 3: ToggleButton/Calendar/ScrollBar/DoubleSpinBox/FontComboBox (gtk::ToggleButton/Calendar/Scrollbar/SpinButton(digits=2)/ComboBoxText; BS_AUTOCHECKBOX|BS_PUSHLIKE/SysMonthCal32/msctls_trackbar32). Round 4: ContextMenu/PopupWindow/Dialog/InputDialog/ProgressDialog/DirectoryDialog (gtk::Menu/Window(Popup)/Dialog×3/FileChooserDialog(SelectFolder)). Round 5: DatePicker/TimePicker/DateTimePicker/ActivityIndicator (GTK composites over real widgets; Win32 SysDateTimePick32 + PBS_MARQUEE).
  - Routing deliberately stays `CustomRequired` for those kinds because only **2 of 9 backends** construct a real OS object; routing them `NativePreferred` would hand the other 7 a state handle under a native-sounding label. The native implementations are reachable on the `NativePreferred` path and are covered by tests.
  - Not nativised (platform has no primitive, so self-drawn is the correct answer): Dial, LCDNumber. `MenuItem` is covered by the real menu path (`create_action`).
  - Round 6 (2026-09-11): Windows `DirectoryDialog` now uses the modern Common Item Dialog — `CoCreateInstance(CLSID_FileOpenDialog, IFileDialog)` + `SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)` + `GetResult`/`GetDisplayName(SIGDN_FILESYSPATH)`, replacing the state-handle fallback (the deprecated `SHBrowseForFolder` was deliberately not used). Wired into `show_widget` so creation stays non-blocking. Requires winapi features `shobjidl` + `winerror`. Compile- and clippy-verified; **runtime verification still needs a Windows machine**.
  - Follow-up 2026-09-11: JNI binding surface is now also auditable as data — `tools/check_jni_signatures.sh` writes `target/qa/jni_binding_map*.json` and `target/qa/jni_android_view_map*.json` (per-method, per-parameter types, library-export status) for every built ABI.
  - Follow-up 2026-09-11: `tools/check_jni_signatures.py` comment-stripping was fixed — a naive `/* */` regex mis-parsed the MIME wildcard string `"*/*"`, swallowing 16 KB of source and hiding 9 exports. It now uses a string-aware Rust comment scanner.
  - CI 2026-09-11: `.github/workflows/android.yml` packages the local scripts into two jobs — a per-ABI `jni-bindings` gate (APK build + sign + JNI signature/export gate + artifact upload) and an `emulator-e2e` job that boots the API 34 x86_64 image with KVM and runs the same runner to assert `RESULT: PASS`. Android now has an unblocked CI lane (BLUE14 item #10).

- [ ] ITEM 4: iOS mobile backend implementation
  - Constraint: iOS backend is reserved in architecture but not implemented here.
  - Blocker: requires UIKit/AppKit mobile lifecycle bridge and CI/build lane for Apple mobile targets.
  - Target completion signal: operational iOS backend with native widget host and lifecycle wiring.

- [ ] ITEM 5: macOS objc2 preview backend graduation
  - Constraint: current `macos-objc2` path is intentionally preview/poll-loop mode.
  - Blocker: requires full objc2 native event-loop parity and migration sign-off from Cocoa backend.
  - Target completion signal: `NativeInteractive` parity and replacement-readiness.

- [ ] ITEM 6: Cross-platform full widget parity matrix closure
  - Constraint: some widgets still rely on default trait-level fallback semantics on at least one backend.
  - Blocker: each backend needs complete `create_*` coverage or explicit unsupported contracts.
  - Target completion signal: no implicit `create_button` fallback for supported controls across all target backends.
  - Correction 2026-09-11 (the original premise does not hold): the `Platform` trait declares 22 `create_*` methods with **zero default bodies**, so a backend cannot silently inherit a wrong default — it either implements the method or fails to compile. The measured `tools/platform_impl_matrix.md` therefore reports `Missing = 0` for the required contract, and the 87 "Placeholder" native-backend grades are `CustomRequired` widgets that are custom-painted **by design** (the project's 原生优先/自绘兔底 policy), with hybrid placeholders measured at 0.
  - What remains genuinely open: per-backend *native* coverage for widgets that could reasonably have one (e.g. Windows SpinBox/ListView/ScrollArea, done 2026-09-11) and explicit "unsupported" contracts where a platform truly has no counterpart (e.g. Android ColorDialog/FontDialog).
  - Target completion signal (restated): every backend either implements a real native `create_*` or documents an explicit unsupported contract, with no widget falling through to a silent state-only path without a recorded reason.
  - Update 2026-09-11 (Round 7, native routing honesty): `DatePicker`/`TimePicker`/`DateTimePicker` no longer delegate to `create_panel` in `src/control_backend/native.rs`; they now call their dedicated platform methods, which already existed on **all 11 backends** (Linux: GTK `MenuButton`+`Popover`+`Calendar` / dual `SpinButton`; Windows: `SysDateTimePick32`). Before the fix those native implementations were unreachable dead code — a direct violation of the "FFI wiring completeness" rule. Routing deliberately stays `CustomRequired` because only Linux(`gtk-native`) and Windows construct a real OS object; marking them `NativePreferred` would hand the other 9 backends a state handle under a native-sounding label.
  - Update 2026-09-11 (Round 7, matrix truthfulness): the "Degradation notes" fallback table in `docs/plans/platform_capability_matrix.md` is now **mechanically derived** from `src/control_backend/native.rs` by `tools/generate_platform_capability_matrix.py` instead of being a hand-typed mirror that had silently drifted (9 widgets were still listed as degraded long after they gained dedicated implementations). `tools/check_platform_capability_matrix.sh` now fails if the committed document diverges from a fresh generation; the check was negatively verified by appending a drift marker.

- [ ] ITEM 7: Host-invisible test coverage (tests excluded from the build by `#[cfg(target_os)]`)
  - Constraint: a `#[cfg(target_os = "...")]`-gated module takes its `#[cfg(test)]` block with it, so those tests are neither run nor type-checked on any other host — a silent zero-coverage state distinct from a failing or `ignored` test.
  - Measured 2026-09-11 (Round 7): 26 tests were in this state (`src/platform/ime_windows.rs` 15, `src/platform/windows/notify.rs` 11). Both are now executable on the host: the `ime_windows` composition state machine is compiled unconditionally (TSF touch points stay internally gated), and the pure Win32 notification-code mapping was moved to the ungated `src/platform/windows_notify.rs` (which the Windows backend now calls, rather than keeping a duplicate copy).
  - What remains genuinely open: other backends have the same shape — `src/platform/ime_macos.rs` (19 tests, pure state machine), `src/platform/macos_objc2/tests.rs` (17), `src/platform/ios/platform_impl.rs` (6), `src/platform/android/platform_impl.rs` (7) + `types.rs` (3), `src/platform/macos/tests.rs` (4), `src/platform/accessibility/windows.rs` (2), and 2 `target_os`-gated tests in `src/control_backend/routing.rs`. These were not touched in Round 7 because reaching them requires the same refactor per module or an actual Apple/Android host.
  - Target completion signal: no pure-logic test is left unreachable on the host; each module either compiles its pure logic unconditionally with OS parts gated, or is explicitly marked as requiring its target.

## Notes

- This file captures **currently constrained** work only (not normal in-progress tasks).
- Active implementation work should continue in `TODO.md`; once blocked by platform/runtime dependency, move item here.
- v1 initialized on 2026-03-03.
