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

#### Signals vs. `EventHandler`: which one to use

Every widget has **two** notification mechanisms, and they answer different questions.
Knowing which is which is the difference between wiring a control correctly and
subscribing to something that never fires.

| | `EventHandler::handle_event` | Signals (`Signal1<T>`, …) |
|---|---|---|
| **Direction** | *Inbound* — the platform or the router hands the widget an `Event`. | *Outbound* — the widget tells the application something happened. |
| **Who implements it** | The widget. It is how a control turns a raw `Event` into its own behaviour (arming, toggling, editing, scrolling). | The application. Callers `connect(..)` a closure. |
| **Multiplicity** | Exactly one handler per widget — it is the widget's own event path. | Many subscribers per signal, all invoked. |
| **Reentrancy** | Called under the widget registry borrow, so an implementor must not mount or unmount widgets. | Callbacks run **unlocked**, so a callback may itself `connect` or `emit` (see `src/signal/`). |
| **Typical question it answers** | "What should this control do when the user presses it?" | "How does the application learn the user pressed it?" |

**The rule of thumb:** a widget `impl EventHandler` to *change its own state*, and
then *emits a signal* so the application can react. A widget that handles input but
never emits is not necessarily wrong (a `Label` has nothing to announce), but a widget
that announces something must do so through a signal.

#### Where the boundary runs inside `BaseWidget`

`BaseWidget` is the one place both mechanisms meet, so the split is defined there and
every control inherits it (`src/widget/base.rs`):

* `BaseWidget` holds seven **primitive** signals — `hover`, `mouse_down`, `mouse_up`,
  `key_down`, `key_up`, `focus_gained`, `focus_lost` — and its `handle_event` emits
  exactly those.
* It deliberately does **not** emit the **semantic** signals `clicked` or `changed`.
  Whether a click happened depends on the control's own gesture: a `Button` needs a
  press *and* a release while still armed, a `CheckBox` toggles, a `Slider` changes its
  value on drag. Only the control knows its gesture, so each emits these itself.

That is why "my widget never emits `clicked`" is usually not a bug: the control either
has no click concept (`Label`, `Panel`) or drives a different signal
(`Slider::value_changed`). Read the concrete control's own signals rather than expecting
the base to supply one.

#### Why they are not merged

`BaseWidget` supporting both is a deliberate design, not two systems that drifted
together. The inbound path (routing every raw platform event into a control) and the
outbound path (letting an application subscribe to *one* fact it cares about) have
different lifetimes, different multiplicity, and different locking requirements — a
single mechanism would have to be the weakest of all three. The cost is that the same
underlying fact ("the button was pressed") can be observed in more than one place; the
benefit is that neither path is forced to carry the other's constraints.

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
