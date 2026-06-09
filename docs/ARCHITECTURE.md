# Rust Widgets — Architecture Overview

> Last updated: 2026-06-09 (BLUE11)

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                     │
│  (app, json_engine, c_abi)                              │
├─────────────────────────────────────────────────────────┤
│                    Widget Layer                          │
│  ~80 widget structs + 10 new mobile widgets             │
│  Each: Widget + Draw + EventHandler + Signals           │
│  Sub-modules: base, input, container, display,          │
│               special, view, advanced, web, menu,       │
│               dialog, new_widgets (BLUE11)               │
├─────────────────────────────────────────────────────────┤
│   Layout        │  Event       │  Style / Theme / Anims │
│   Absolute/Box  │  Mouse/Touch │  ThemeManager          │
│   Flow/Form     │  Keyboard    │  ThemeOverrides        │
│   Grid/Splitter │  Focus       │  Easing Functions      │
│   Stack/Uniform │  IME         │  - Keyframes (WIP)     │
│                  │  Timer       │  - Transitions (WIP)   │
├─────────────────────────────────────────────────────────┤
│                  Rendering Pipeline                      │
│  RenderCommand → PaintBackend (Software/SVG/WGPU)       │
│  RenderScene → SceneLayer → compose_with_backend()      │
│  GpuRenderer trait → WgpuRenderer (w/ WgpuDrawCommand)  │
├─────────────────────────────────────────────────────────┤
│                  Control Backend                        │
│  Creates native platform widgets OR custom-painted      │
│  ControlRoutePreference: NativePreferred / CustomRequired│
├─────────────────────────────────────────────────────────┤
│                  Platform Layer                          │
│  Windows (Win32) │ macOS (Cocoa 0.24) │ Linux (GTK)     │
│  macOS (objc2 WIP) │ Wayland (WIP)                      │
│  iOS (state-only) │ Android (JNI) │ HarmonyOS (state)   │
│  Embedded (stub)                                          │
├─────────────────────────────────────────────────────────┤
│                  Core Infrastructure                    │
│  ObjectId, Rect/Size/Point, Color, Font, Signal, Error  │
│  i18n, Accessibility, Gesture Recognition, GPU Mgmt     │
└─────────────────────────────────────────────────────────┘
```

## Key Design Decisions

### Native-First, Custom-Fallback
Each widget has a `ControlRoutePreference`: `NativePreferred` for platform-native controls
(Button, Label, etc.) and `CustomRequired` for custom-drawn widgets (Canvas, Chart, etc.).

### Dual Rendering Pipeline
Render commands flow through a unified `RenderCommand` enum but are executed by
different backends: `SoftwarePaintBackend` (CPU raster), `SvgPaintBackend` (SVG output),
or `WgpuRenderer` (GPU via wgpu — WIP for full feature parity).

### Signal System
Widgets use generic signals (`Signal1<T>`, `GenericSignal`) for event notification.
Signals are emitted on state changes and can be connected to closures or other widgets.

### Device Profiles
Four mutually-exclusive profiles: `desktop` (default), `tablet`, `mobile`, `embedded`.
Interaction add-ons (`touch`, `holographic`, `projection`) compose on top.

## Module Map

| Directory | Purpose | File Count |
|-----------|---------|------------|
| `src/widget/` | All widget structs + traits | ~101 .rs files |
| `src/platform/` | Platform backends (8 platforms) | ~40 .rs files |
| `src/render/` | Rendering pipeline | ~20 .rs files |
| `src/event/` | Event system | ~9 .rs files |
| `src/layout/` | Layout engines | ~10 .rs files |
| `src/control_backend/` | Control creation routing | ~9 .rs files |
| `src/style/` | Animation + styling | ~5 .rs files |
| `src/theme/` | Theme management | ~3 .rs files |
| `src/gpu/` | GPU adapter management | ~3 .rs files |
| `src/quality/` | Adaptive quality system | ~5 .rs files |
| `src/core/` | Core types (Rect, Color, etc.) | ~9 .rs files |
| `src/wgpu_backend/` | WGPU renderer | ~5 .rs files |

## BLUE11 Improvements

### Completed
- **R10.1-R10.7**: Switch, SearchBox, Chip, Badge, SkeletonLoader, FAB, PullToRefresh
- **R10.8-R10.13**: BottomSheet, BottomNavigationBar, NavigationDrawer, AppBar, MobileDatePicker
- **R4.1**: Cargo.toml enhanced (authors, categories, include/exclude)
- **R4.2**: deny.toml created

### In Progress
- **R1.5-R1.6**: macOS objc2 migration (macos_objc2/ preview exists)
- **R3.1**: Test coverage expansion
- **R9.4**: pipeline/containers.rs splitting
