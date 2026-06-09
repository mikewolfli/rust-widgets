# Widget Gallery — rust_widgets

> Last updated: 2026-06-09 (BLUE11)

A visual reference of all available widgets in the rust_widgets library.

## Base Widgets

| Widget | Description | SVG Preview |
|--------|------------|-------------|
| Button | Clickable button with text label | `cargo run --example demo_button` |
| Label | Static text display | — |
| CheckBox | Checkable box with label | — |
| RadioButton | Mutually exclusive option | — |
| ToggleButton | Sticky on/off button | — |

## Input Widgets

| Widget | Description | Status |
|--------|------------|--------|
| LineEdit | Single-line text input | ✅ |
| TextEdit | Multi-line text editor | ✅ |
| ComboBox | Drop-down selection | ✅ |
| SpinBox | Numeric spin control | ✅ |
| Slider | Horizontal/vertical slider | ✅ |
| SearchBox | Search input with clear button | ✅ (BLUE11) |
| TagInput | Tag/chip creation input | ✅ (BLUE11) |

## Container Widgets

| Widget | Description | Status |
|--------|------------|--------|
| TabWidget | Tabbed panel container | ✅ |
| Splitter | Resizable split panes | ✅ |
| GroupBox | Grouped frame with title | ✅ |
| ScrollArea | Scrollable content area | ✅ |
| CollapsiblePane | Expandable section | ✅ |

## Display Widgets

| Widget | Description | Status |
|--------|------------|--------|
| ProgressBar | Progress indicator | ✅ |
| ScrollBar | Scroll position indicator | ✅ |
| LCDNumber | Digital display | ✅ |
| SkeletonLoader | Content placeholder | ✅ (BLUE11) |

## New Widgets (BLUE11 R10)

| # | Widget | Category | Status |
|---|--------|----------|--------|
| 1 | Switch | Popular Control | ✅ |
| 2 | SearchBox | Popular Control | ✅ |
| 3 | Chip/Tag | Popular Control | ✅ |
| 4 | Badge | Popular Control | ✅ |
| 5 | SkeletonLoader | Popular Control | ✅ |
| 6 | FAB | Popular Control | ✅ |
| 7 | PullToRefresh | Mobile | ✅ |
| 8 | BottomSheet | Mobile | ✅ |
| 9 | BottomNavigationBar | Mobile | ✅ |
| 10 | NavigationDrawer | Mobile | ✅ |
| 11 | AppBar/TopBar | Mobile | ✅ |
| 12 | ContextMenu | Mobile | ✅ |
| 13 | MobileDatePicker | Mobile | ✅ |
| 14 | Avatar | Popular Control | ✅ |
| 15 | Rating | Popular Control | ✅ |
| 16 | Stepper | Popular Control | ✅ |
| 17 | Divider | Popular Control | ✅ |
| 18 | Carousel | Popular Control | ✅ |
| 19 | EmptyState | Popular Control | ✅ |
| 20 | TagInput | Desktop Advanced | ✅ |
| 21 | ColorWell | Popular Control | ✅ |
| 22 | QRCode | Popular Control | ✅ |
| 23 | MasonryLayout | Layout | ✅ |
| 24 | CupertinoSwitch | iOS Style | ✅ |
| 25 | MaterialSnackbar | Android Style | ✅ |
| 26 | AdaptiveScaffold | Cross-platform | ✅ |
| 27 | PropertyGrid | Desktop Advanced | ✅ |
| 28 | WizardDialog | Desktop Advanced | ✅ |
| 29 | ImePreedit | Input Support | ✅ |

## Rendering

All widgets support SVG output via `render_widget_to_svg()` for accurate
visual representation and testing.
