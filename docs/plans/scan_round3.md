# Scan Round 3: Deep Code Review Report

**Project:** rust_widgets v0.6.1
**Date:** 2024-07-15
**Scope:** `control_backend/`, `render/`, `chart/`, `theme/`

---

## 1. `src/control_backend/` — Backend Abstraction

### 1.1 `trait_def.rs` — `ControlBackend` Trait

#### 🔴 Issue: Some required trait methods have no default implementation — but they have no `todo!()`

All methods are declared with no default implementation except `poll_widget_triggered()` and `show_widget()`/`hide_widget()` (which delegate to other methods). This is acceptable because implementations must provide them.

#### ✅ `poll_widget_triggered()` — Good default, delegates to `poll_widget_trigger_event()`

#### ✅ `show_widget()` / `hide_widget()` — Good defaults calling `set_widget_visible()`

### 1.2 `native.rs` — `NativeControlBackend`

#### 🔴 ISSUE NW-1: Wrong surrogate mapping — `create_dialog()` uses `create_message_box()`

```rust
// native.rs L218-228
fn create_dialog(...) -> ObjectId {
    get_platform().create_message_box(parent, title, "", x, y, width, height)
}
```
A generic `Dialog` is mapped to `create_message_box` with an empty `text` parameter. This is semantically wrong — a dialog and a message box are different UI primitives. A dialog should use a dedicated `create_dialog()` on the platform layer, or at minimum should not conflate it with a message box.

**Severity:** Medium — will produce incorrect native widget types.

#### 🔴 ISSUE NW-2: Wrong surrogate mapping — `create_toggle_button()` uses `create_checkbox()`

```rust
// native.rs L430-440
fn create_toggle_button(...) -> ObjectId {
    get_platform().create_checkbox(parent, text, x, y, width, height)
}
```
`ToggleButton` is visually and behaviorally distinct from a `CheckBox`. Surrogate mapping creates a checkbox with toggle-button semantics, which is wrong.

**Severity:** Medium

#### 🔴 ISSUE NW-3: Wrong surrogate mapping — `create_text_edit()` and `create_rich_edit()` both use `create_line_edit()`

```rust
// native.rs L285-295, L296-306
fn create_text_edit(...) -> ObjectId {
    get_platform().create_line_edit(parent, text, x, y, width, height)
}
fn create_rich_edit(...) -> ObjectId {
    get_platform().create_line_edit(parent, text, x, y, width, height)
}
```
TextEdit (multi-line) and RichEdit (rich text, multi-line) are mapped to LineEdit (single-line, plain text). This loses multi-line functionality.

**Severity:** High — functional regression.

#### 🔴 ISSUE NW-4: Wrong surrogate mapping — `create_scroll_bar()` uses `create_slider()`

```rust
// native.rs L337-346
fn create_scroll_bar(...) -> ObjectId {
    get_platform().create_slider(parent, x, y, width, height)
}
```
A scroll bar is not a slider. They have different interaction patterns and visual representations.

**Severity:** Medium

#### 🔴 ISSUE NW-5: Wrong surrogate mapping — `create_tree_view()` uses `create_list_box()`

```rust
// native.rs L327-336
fn create_tree_view(...) -> ObjectId {
    get_platform().create_list_box(parent, x, y, width, height)
}
```

**Severity:** Medium

#### 🔴 ISSUE NW-6: Wrong surrogate mapping — `create_scroll_area()`, `create_dock_panel()`, `create_group_box()`, `create_tab_widget()`, `create_splitter()`, `create_stack_widget()`, `create_mdi_area()`, `create_canvas()`, `create_table()`, `create_grid()`, `create_chart()`, `create_wizard()`, `create_date_picker()`, `create_time_picker()`, `create_date_time_picker()`, `create_toolbox()`, `create_dock_widget()`, `create_web_view()`, `create_calendar()`, and all `web_engine_*` methods all map to `create_panel()`

This is a massive group of 20+ methods that all redirect to `create_panel(parent, x, y, width, height)`. These widgets are completely different primitives but get the same panel underneath. The `web_engine_*` items (Page, Settings, DownloadItem, CookieStore, WebChannel, FindTextResult, Notification, ScriptDialog, ContextMenuRequest) mapping to a simple Panel is particularly egregious — these are not UI widgets at all in the traditional sense.

**Severity:** High — severe loss of native platform fidelity.

#### 🟡 ISSUE NW-7: Title parameter silently dropped — `create_file_dialog()`, `create_color_dialog()`, `create_font_dialog()`, `create_directory_dialog()`

```rust
// native.rs L241-273
fn create_file_dialog(&self, parent: ObjectId, _title: &str, ...) -> ObjectId {
    get_platform().create_file_dialog(parent, x, y, width, height)
}
```
The `_title` parameter is accepted (to satisfy the trait) but silently dropped. The underlying platform call doesn't accept a title.

**Severity:** Low — cosmetic, but title is important for dialogs.

#### 🟡 ISSUE NW-8: `create_canvas()` creates a Panel — not a Canvas

```rust
// native.rs L418-420
fn create_canvas(...) -> ObjectId {
    get_platform().create_panel(parent, x, y, width, height)
}
```
The `Canvas` widget type should provide a drawing surface. Mapping to `Panel` loses that capability.

**Severity:** High — a Canvas that isn't a canvas breaks the graphics API contract.

#### 🟡 ISSUE NW-9: `create_chart()` creates a Panel — no chart rendering support

```rust
// native.rs L427-429
fn create_chart(...) -> ObjectId {
    get_platform().create_panel(parent, x, y, width, height)
}
```
Charts are rendered through the `chart/` module, but the native backend creates plain panels.

**Severity:** Low — charts are expected to use custom rendering anyway.

#### 🟡 ISSUE NW-10: `create_lcd_number()` creates a Label with text "0"

```rust
// native.rs L634-643
fn create_lcd_number(...) -> ObjectId {
    get_platform().create_label(parent, "0", x, y, width, height)
}
```
This hardcodes the display text as `"0"` and ignores the widget's actual value. The text should come from the widget's state, not be hardcoded at creation.

**Severity:** Medium — incorrect initial state.

### 1.3 `custom.rs` — `CustomPaintControlBackend`

#### 🔴 ISSUE C-1: `alloc_widget_id()` may overflow silently

```rust
// custom.rs L23-31
fn alloc_widget_id(&self) -> ObjectId {
    let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
    let id = state.next_widget_id;
    state.next_widget_id += ObjectId::from(1u64);
    id
}
```
`next_widget_id` is `ObjectId` which wraps a `u64`. Incrementing past `u64::MAX` will wrap to 0, potentially creating duplicate IDs. While practically unlikely, there's no overflow check or graceful handling.

**Severity:** Low — theoretical.

#### 🔴 ISSUE C-2: Thread safety — every method acquires `self.state.lock()`

Every single method in the `ControlBackend` impl for `CustomPaintControlBackend` acquires `self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())`. This works but can cause contention under heavy load. Additionally, the `unwrap_or_else` pattern silently continues after a poisoned mutex, which may hide bugs.

**Severity:** Low — acceptable pattern but worth noting.

#### ✅ All 90+ `create_*` methods are fully implemented with property storage — no stubs.

The custom backend is the most complete implementation, storing widget properties (parent, geometry, kind) for all widget types. This is commendable.

### 1.4 `routing.rs` — `route_preference_for_widget_kind()`

#### 🔴 ISSUE R-1: Mismatch between trait_def and routing — `create_table` expects CustomRequired but `create_table` in native maps to Panel

In routing.rs, `Table` → `CustomRequired`. In native.rs, `create_table` → `create_panel()`. These two are inconsistent — if the routing says CustomRequired, why is there a native implementation at all?

**Severity:** Low — the routing will correctly send to custom backend in hybrid mode, so the native impl is dead code.

#### 🔴 ISSUE R-2: `StackedWidget`, `Action`, `ToolButton`, `ToolBox` appear in TWO match arms

```rust
// routing.rs
WidgetKind::StackedWidget | WidgetKind::Action | WidgetKind::ToolButton | WidgetKind::ToolBox
    => ControlRoutePreference::CustomRequired,
```
AND these same variants also appear in the first match arm:
```rust
WidgetKind::... | WidgetKind::ToolBox | ...
    => ControlRoutePreference::NativePreferred,
```

Wait — let me verify. Looking at routing.rs lines 13-50, the first arm covers many widgets including `ToolBox`, but the second arm separately covers `StackedWidget | Action | ToolButton | ToolBox`. So `ToolBox` is matched in BOTH arms.

Actually, `WidgetKind::ToolBox` appears only in the second arm (`CustomRequired`) in the current code. Let me double-check — it's NOT in the first arm. My mistake. The first arm ends at `WidgetKind::RibbonBar`. So no actual double-match.

**Verdict:** Clear. No issue.

### 1.5 `dispatcher.rs` — Feature Gate Handling

#### 🟡 ISSUE D-1: When both features are disabled, falls back to Native

```rust
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
```
This compiles fine because `native_control_backend()` is always defined (not feature-gated), but if both features are disabled, presumably the user expects no backend. The fallback to native might be surprising.

**Severity:** Low — existing behavior is documented by `active_control_policy()` returning `"native-strict"`.

#### 🟡 ISSUE D-2: `custom_control_backend()` returns `OnceLock<CustomPaintControlBackend>` but never drops

`OnceLock::get_or_init` runs once and the instance lives forever. If `CustomPaintControlBackend` holds significant resources, they'll never be freed.

**Severity:** Informational — `OnceLock` pattern is standard for singletons.

---

## 2. `src/render/` — Rendering Pipeline

### 2.1 `render/core/command.rs` — `RenderCommand`

#### ✅ Good coverage — 21 command variants for FillRect, DrawRect, FillRoundedRect, DrawRoundedRectStroke, DrawLine, FillCircle, DrawCircle, DrawText, DrawImage, PushClip, PopClip.

#### 🔴 ISSUE RC-1: Missing `DrawEllipse` command

There are `FillCircle`, `DrawCircle`, `DrawCircleStroke`, `FillCircleAA`, etc. But there is NO `DrawEllipse` or `FillEllipse` variant — the `ChartContext` trait in `chart/types.rs` requires `draw_ellipse()` but the `RenderCommand` enum has no matching command. The SVG context handles it, but the software surface cannot render ellipses natively.

**Severity:** Medium — charts that use ellipses will render incorrectly in software mode.

#### 🔴 ISSUE RC-2: Missing `DrawPolygon` / `FillPolygon` command

The `ChartContext::draw_polygon()` method has no corresponding `RenderCommand`. Chart rendering uses polygons for area charts, pie charts, and arcs, but these are translated to `DrawLine` workarounds or simply lost when going through the pipeline.

**Severity:** Medium — area chart fills and pie chart slices have no software path.

#### 🔴 ISSUE RC-3: Missing `DrawPath` command

`ChartContext::draw_path()` has no matching `RenderCommand`.

**Severity:** Low — path can be approximated with lines.

#### 🔴 ISSUE RC-4: Missing `DrawArc` command

`ChartContext::draw_arc()` has no matching `RenderCommand`. The SVG backend handles it via polygons, but the software surface has no native arc support.

**Severity:** Medium

### 2.2 `render/backend/batch.rs` — `BatchRenderer`

#### 🔴 ISSUE B-1: `Translate` and `SetOpacity` mapped to no-op FillRect

```rust
// batch.rs L216-219
BatchCommand::Translate { .. } | BatchCommand::SetOpacity { .. } => {
    RenderCommand::FillRect {
        rect: Rect::new(0, 0, 0, 0),
        color: Color::TRANSPARENT,
    }
}
```
These two batch commands are silently ignored during replay. They translate to a zero-sized transparent fill rect — effectively a no-op. Any scene that depends on translation or opacity will produce incorrect output.

**Severity:** Medium — missing transform/opacity support in batch replay.

#### 🔴 ISSUE B-2: Batch replay clones the entire BatchState

```rust
// batch.rs L232-235
fn replay(&mut self, id: BatchId) {
    let state = self.batch_state.clone();
    state.replay(self, id);
}
```
Cloning the entire `BatchState` (including all batches and image data) on every `replay()` call is a performance concern.

**Severity:** Medium — performance issue for batch-heavy workloads.

### 2.3 `render/backend/scene.rs` — RenderScene

#### 🔴 ISSUE S-1: `compose_to()` ignores the returned `AutoRenderBackend`

```rust
// scene.rs L137-141
pub fn compose_to_config(...) {
    let _ = self.compose_to_config_auto(surface, clear, config);
}
```
The auto backend selection result is explicitly discarded, so callers have no idea whether GPU or CPU was used.

**Severity:** Low — intentional API choice.

#### 🔴 ISSUE S-2: Feature-gated quality functions compile even without `quality-management` feature

```rust
#[cfg(feature = "quality-management")]
pub fn current_quality_level() -> ... { ... }
// BUT the module re-exports them unconditionally in render/mod.rs
```
Actually — looking at `render/mod.rs`, the functions are re-exported from `backend` which is not feature-gated. However, the functions themselves are feature-gated, so they'd only exist when the feature is active. This is fine.

**Verdict:** No issue.

#### 🔴 ISSUE S-3: `compose_scene_to_surface_software()` copies the entire buffer

```rust
// scene.rs L166
surface.buffer = backend.surface.buffer;
```
This moves ownership of the entire pixel buffer from a temporary backend to the surface. The buffer `BackBuffer` struct contains `front: Vec<u8>` and `back: Vec<u8>`. For a 1920×1080 surface, that's ~16MB moved on every frame.

**Severity:** Low — performance concern but standard pattern.

### 2.4 `render/backend/paint.rs` — PaintBackend

#### ✅ Good — trait is clean, `SoftwarePaintBackend` has full command dispatch.

### 2.5 `render/pipeline/` — Visual Pipeline Functions

#### 🔴 ISSUE PV-1: All `append_*_visual_commands` functions are deprecated

Every single visual pipeline function carries `#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]`. This indicates the entire pipeline approach is deprecated, and yet all widgets still rely on it. No migration path is provided.

**Severity:** High — the primary rendering path is deprecated, causing confusion.

#### 🔴 ISSUE PV-2: Missing visual commands for many widget types

The following widgets have NO `append_*_visual_commands` function in the pipeline:
- `DatePicker`, `TimePicker`, `DateTimePicker` (only via stub Panel mapping in native.rs)
- `WebView`, `WebEngineView`, `WebEnginePage`, `WebEngineSettings`, `WebEngineDownloadItem`, `WebEngineCookieStore`, `WebEngineWebChannel`, `WebEngineFindTextResult`, `WebEngineNotification`, `WebEngineScriptDialog`, `WebEngineContextMenuRequest` (no pipeline visual commands)
- `Action`, `ToolBox`, `ToolButton` (no pipeline visual commands, only special rendering)

**Severity:** High — these widgets have no visual representation in the custom paint pipeline.

#### 🔴 ISSUE PV-3: `append_menu_visual_commands()` uses `menu.items()` but MenuItem fields are accessed directly

```rust
// menu_toolbar.rs L93-L127
if item.separator { ... }
if item.checkable { ... }
if item.checked { ... }
```
The code assumes `item` has fields like `.separator`, `.checkable`, `.checked`, `.shortcut`, `.has_submenu`. If the `MenuItem` type doesn't expose these as public fields, this won't compile. This should be verified against the widget types.

**Severity:** Medium — potential compilation failure if widget types change.

### 2.6 `render/gpu/mod.rs` — GpuRenderer Trait

#### 🔴 ISSUE GPU-1: Trait is defined but no actual GPU renderer implementation exists in this module

The `GpuRenderer` trait and `GpuCapability` enum are defined here, but the implementations are in `src/wgpu_backend/`. The module comment says "This module re-exports its types" but actually DEFINES them, not re-exports.

**Severity:** Low — the docs are misleading but functionality is intact.

### 2.7 `render/quality/adaptive.rs` — AdaptiveRenderer

#### ✅ Good implementation — complete with unit tests.

#### 🟡 ISSUE AQ-1: `println!` in production code

```rust
// adaptive.rs L125
println!("{} quality to {:?} (FPS: {:.1}, Avg frame time: {:.3}ms)", ...);
```
The adaptive renderer logs quality changes via `println!` instead of a proper logging framework. In embedded/headless environments, this may be undesirable.

**Severity:** Low — should use `log::info!` instead.

### 2.8 `render/web/` — WebView and WebEngine

#### 🟡 ISSUE W-1: `WebView` struct has no actual rendering logic

```rust
// web/view.rs
pub fn request_redraw(&self) { /* no-op */ }
pub fn set_scroll_offset(&mut self, _offset: Point) { /* no-op */ }
```
The `WebView` rendering adapter is essentially a placeholder — `request_redraw()` does nothing and `set_scroll_offset()` is a no-op.

**Severity:** Low — the real rendering happens in the WebEngine widget itself.

### 2.9 `render/text_cache.rs` — TextCache and GlyphCache

#### 🔴 ISSUE TC-1: `TextCache::get()` double-borrows the HashMap

```rust
// text_cache.rs L91-97
pub fn get(&mut self, key: &TextKey) -> Option<&CachedText> {
    self.current_timestamp += 1;
    if let Some(cached) = self.cache.get(key) {
        if self.is_expired(cached) {
            self.cache.remove(key);  // BORROW ERROR? self.cache is already borrowed
            self.misses += 1;
            return None;
        }
```
Actually, looking more carefully: the `if let Some(cached) = self.cache.get(key)` borrows `self.cache` immutably, then `self.cache.remove(key)` tries to borrow mutably. This will NOT compile. Wait — let me re-check. The `if let` binding `cached` borrows from `self.cache.get(key)`, and that borrow lives for the scope of the `if let`. Inside that scope, `self.cache.remove(key)` would be a mutable borrow while `cached` (immutable borrow of `self.cache`) exists.

Actually, the `if let Some(cached) = ...` only borrows through the `if` condition test. If `Some(cached)` matches, then `cached` is a reference borrowed from the HashMap. Inside `if self.is_expired(cached)`, `cached` is still borrowed. Then `self.cache.remove(key)` tries to mutably borrow while `cached` is still borrowd.

This is a **potential borrow-check error** that should cause a compilation failure. If it compiles, it's because NLL (Non-Lexical Lifetimes) is able to see that `cached` is not used after `is_expired()` returns. Let me verify by checking Rust edition — with edition 2021 and NLL, this should compile because `cached`'s borrow ends after `is_expired(cached)` evaluates.

**Verdict:** Compiles with NLL. No issue.

#### 🔴 ISSUE TC-2: `TextCache::get_mut()` same pattern — compiles due to NLL.

**Verdict:** Same as TC-1. No issue.

#### 🟡 ISSUE TC-3: TTL expiration uses timestamp counter, not real time

```rust
// text_cache.rs L177-179
fn is_expired(&self, cached: &CachedText) -> bool {
    let age = self.current_timestamp.saturating_sub(cached.timestamp);
    age > self.config.ttl_seconds * 60
}
```
`current_timestamp` is incremented on every `get()`/`get_mut()` call, NOT based on actual wall-clock time. The comparison `age > ttl_seconds * 60` means entries expire after `ttl_seconds * 60` accesses, not seconds of real time. The comment/doc says "ttl_seconds" but it's actually "ttl_accesses".

**Severity:** Medium — cache TTL is misleading and functionally wrong for time-based expiration.

---

## 3. `src/chart/` — Chart System

### 3.1 `chart/types.rs` — Types and Traits

#### 🟡 ISSUE CH-1: `ChartType::create_chart()` creates default charts with no context

```rust
// types.rs L35-44
pub fn create_chart(&self) -> Box<dyn Chart> {
    match self {
        ChartType::Line => Box::new(crate::chart::charts::LineChart::new()),
        ChartType::Bar => Box::new(crate::chart::charts::BarChart::new()),
        ...
    }
}
```
This creates charts with default settings and no data. The caller must then call `add_series()`, `set_title()`, etc. This is fine but there's no way to pass initial configuration.

**Severity:** Low — acceptable factory pattern.

#### 🔴 ISSUE CH-2: `ChartContext::draw_polygon()` and `draw_path()` have no software render path

As noted in RC-1/RC-2, the `ChartContext` trait requires `draw_polygon()` and `draw_path()`, but the `RenderCommand` enum has no polygon or path commands. The SVG context handles them; the `MemoryChartContext` records them as strings. But in the actual software pipeline, these calls either get mapped to lines/rects or are ignored.

**Severity:** High — area charts, pie charts, and any polygon-based charting won't render correctly on the software backend.

### 3.2 `chart/charts.rs` — Chart Implementations

#### 🔴 ISSUE CH-3: PieChart `set_x_axis_label()` and `set_y_axis_label()` are silent no-ops

```rust
// charts.rs L670-673
fn set_x_axis_label(&mut self, _label: String) { /* Not used in pie chart */ }
fn set_y_axis_label(&mut self, _label: String) { /* Not used in pie chart */ }
```
The Chart trait requires these methods, and PieChart accepts labels silently without storing them. If a caller calls `set_x_axis_label()` on a PieChart, there's no indication that it does nothing.

**Severity:** Low — acceptable for pie charts, but documentation should note this.

#### 🔴 ISSUE CH-4: AreaChart stacked mode uses accumulation that may produce incorrect baselines

```rust
// charts.rs L1077-1092
if self.stacked {
    if accum.as_ref().map_or(true, |a| a.len() != series.data.len()) {
        accum = Some(vec![0.0_f64; series.data.len()]);
    }
    let acc = accum.as_mut().unwrap();
    series.data.iter().enumerate().map(|(i, p)| {
        let stacked_y = acc[i] + p.y;
        acc[i] = stacked_y;
        stacked_y
    }).collect()
}
```
In stacked mode, each series accumulates on the previous one. But **all series must have the same number of data points at the same x positions** for stacking to make sense. If one series has fewer points, the accumulator is reset (`len()` mismatch check), breaking the stack.

Additionally, the accumulation assumes data points are in order and correspond by index (not by x-value). If series have points at different x-values, the stacking will be incorrect.

**Severity:** Medium — stacked area charts will produce wrong results for mismatched data series.

#### 🔴 ISSUE CH-5: Hardcoded axis label positioning may overflow

```rust
// charts.rs L320-326
context.draw_text(
    &self.x_axis_label,
    Point::from_f32(layout.plot_x + layout.plot_w * 0.5 - 28.0, layout.plot_y + layout.plot_h + 36.0),
    11.0, ...
);
```
The x-axis label position is offset by a hardcoded `-28.0` pixels from center, and `36.0` pixels below the plot. For short labels this looks fine, but long labels will overflow the chart rect without clipping.

**Severity:** Low — cosmetic.

#### 🟡 ISSUE CH-6: All chart `draw()` methods accept `&self` (immutable)

```rust
fn draw(&self, rect: Rect, context: &mut dyn ChartContext);
```
Charts draw with an immutable reference to self, which means they cannot cache computed layouts or perform lazy initialization. This is fine for simple charts but wasteful for complex ones that recompute tick positions on every frame.

**Severity:** Low — design choice.

### 3.3 `chart/svg.rs` — SVG export

#### ✅ Good — full implementation of all `ChartContext` methods for SVG output.

#### 🟡 ISSUE SVG-1: `save()` method uses blocking filesystem I/O

```rust
// svg.rs L44-46
pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
    fs::write(path, self.to_svg_string())
}
```
This performs synchronous file I/O in what may be called from a rendering context. No async alternative is provided.

**Severity:** Low — acceptable for utility export.

#### 🔴 ISSUE SVG-2: `draw_arc()` renders as filled polygon

```rust
// svg.rs L153-177
fn draw_arc(&mut self, center, radius, start_angle, end_angle, color) {
    // Starts from center, then arc boundary points
    pts.push(Point::from_f64(cx, cy));
    ...
}
```
The arc is rendered as a filled polygon (pie slice from center to arc boundary). The `draw_arc` name suggests it should draw just the arc line, not a filled pie segment. This is inconsistent with the semantic expectation.

**Severity:** Medium — `draw_arc` semantically means a curved line, but implementation creates a filled pie wedge.

### 3.4 `chart/layout.rs` — ChartLayout

#### ✅ Good — single-child layout implementation is complete.

---

## 4. `src/theme/` — Theme System

### 4.1 `theme/types.rs` — Theme Data Types

#### ✅ Good — well-structured with all necessary types (Theme, Colors, Fonts, Spacing, Borders, ThemeOverrides, ThemeStyleToken).

#### 🔴 ISSUE TH-1: `Color::from_hex()` and `Color::to_hex()` are defined in types.rs but these methods might conflict with core Color

```rust
// types.rs L31-37
impl Color {
    pub fn from_hex(hex: &str) -> Result<Self, String> { ... }
    pub fn to_hex(&self) -> String { ... }
}
```
These methods are defined on `Color` inside the theme module, but `Color` is defined in `crate::core::Color`. If `Color::from_hex()` is also defined in the core module, this would cause a duplicate method error. If not, these methods are only available when `theme/types.rs` is in scope.

Actually, in Rust, you can extend a type from another crate or module with `impl` blocks. These methods would be available whenever `theme::types` (or `theme`) is imported. This is fine.

**Verdict:** No issue — valid extension methods.

### 4.2 `theme/manager.rs` — ThemeManager

#### 🔴 ISSUE TH-2: `resolve_style()` has no caching

Every call to `resolve_style(class_name)` recomputes the `WidgetStyle` from scratch. For widgets that call `resolve_style` every frame, this is wasteful. The resolved style for a given theme + class combination could be cached.

**Severity:** Low — performance concern.

#### 🔴 ISSUE TH-3: `resolve_style()` handles limited widget class names

```rust
// manager.rs L93-108
match class_name {
    "button" | "toggle" => (Some(theme.colors.primary), Some(Color::WHITE), Some(theme.colors.primary)),
    "label" => (Some(Color::TRANSPARENT), Some(theme.colors.foreground), None),
    "input" | "lineedit" | "textedit" => (...),
    "slider" | "progress" => (...),
    "panel" | "window" | "dialog" => (...),
    "checkbox" | "radio" => (...),
    _ => (Some(theme.colors.background), Some(theme.colors.foreground), Some(theme.colors.secondary)),
}
```
The class name mapping only recognizes 12 specific class name strings. All other classes fall through to the `_` default. Custom widget classes or widget types not in this list get generic background/foreground colors instead of semantically appropriate defaults.

Missing mappings include:
- `"treeview"`, `"listview"`, `"listbox"` — need distinct backgrounds
- `"scrollbar"`, `"scrollarea"` — need distinct track/thumb colors
- `"tab"`, `"tabwidget"` — need tab-specific styling
- `"splitter"` — needs splitter handle styling
- `"combobox"` — needs combo-specific styling
- `"menubar"`, `"menu"`, `"menuitem"` — menus need distinct styling
- `"toolbar"`, `"statusbar"` — toolbars need different backgrounds
- `"spinbox"`, `"doublespinbox"` — spin buttons need styling
- `"calendar"`, `"datepicker"`, `"timepicker"` — date/time pickers need calendar styling
- `"webview"` — needs distinct background

**Severity:** Medium — limited theme coverage; many widgets get generic fallback styling.

#### 🔴 ISSUE TH-4: `resolve_style()` always sets shadow if theme says so

```rust
// manager.rs L80-86
let shadow = if theme.borders.shadow {
    Some(Shadow { x: 0, y: 2, blur: 6, color: Color::rgba(0, 0, 0, 60) })
} else {
    None
};
```
The shadow is a single hardcoded value applied to ALL widgets when `borders.shadow` is true. There's no per-class shadow customization. Elevation-sensitive widgets (dialogs, popups, menus) should have larger shadows than basic controls (buttons, labels).

**Severity:** Low — uniform shadow is acceptable for initial implementation.

#### 🟡 ISSUE TH-5: `load_theme()` error type is overly broad

```rust
pub fn load_theme(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>>
```
Uses `Box<dyn std::error::Error>` instead of a concrete error type. While flexible, this makes it harder for callers to match on specific errors.

**Severity:** Low — standard pattern.

#### 🟡 ISSUE TH-6: `save_theme()` error type uses `String`

```rust
pub fn save_theme(&self, path: &str) -> Result<(), String>
```
Uses `String` for errors instead of a proper error type. Lacks the `std::error::Error` trait implementation.

**Severity:** Low — works but not idiomatic.

---

## 5. Cross-Cutting Issues

### 🔴 ISSUE CC-1: `todo!()` / `unimplemented!()` / `unreachable!()` — Found nowhere in scanned files ✅

All four scanned areas are free of panic macros (`todo!`, `unimplemented!`, `unreachable!`, `panic!`). This is excellent.

### 🔴 ISSUE CC-2: `unreachable!()` in linked code — verify `cached_wgpu_renderer()`

```rust
// render/backend/scene.rs L203-206
fn cached_wgpu_renderer() -> Option<&'static WgpuRenderer> {
    static RENDERER: OnceLock<Option<WgpuRenderer>> = OnceLock::new();
    RENDERER.get_or_init(|| WgpuRenderer::new().ok()).as_ref()
}
```
Returns `None` if `WgpuRenderer::new()` fails. The caller `compose_scene_to_surface_wgpu()` returns `Err(GpuRenderError::RendererUnavailable)`. No panic path.

### 🔴 ISSUE CC-3: Dead code — `#[allow(dead_code)]` found in multiple files

- `render/web/engine.rs:1` — `#![allow(dead_code)]` (entire module)
- `render/web/view.rs:1` — `#![allow(dead_code)]` (entire module)
- `render/backend/batch.rs:1` — `#![allow(dead_code)]`
- `render/pipeline/special.rs` — functions individually annotated `#[allow(dead_code)]`
- `render/pipeline/mod.rs` — routing functions gated behind `unstable-pipeline-routing` feature

The web rendering modules are entirely suppressed for dead-code warnings, indicating they may be unused.

**Severity:** Medium — indicates unused or dead code paths.

### 🔴 ISSUE CC-4: `#[allow(deprecated)]` used extensively in render/mod.rs re-exports

All `append_*_visual_commands` functions are re-exported with `#[allow(deprecated)]`. This means new code that imports these functions won't get deprecation warnings. The deprecation is being actively hidden.

**Severity:** Medium — hides the deprecation from downstream users.

### 🔴 ISSUE CC-5: Missing documentation

All scanned modules have module-level doc comments (`//!`) and most public functions have doc comments (`///`). However:

- `render/backend/batch.rs` — `BatchState::new()`, `BatchState::translate_command()` have incomplete docs
- `render/backend/surface.rs` — `SoftwareSurface` struct fields have no documentation
- `control_backend/types.rs` — `CustomControlState`, `CustomWidgetProperties` fields are undocumented
- `chart/charts.rs` — `BarChart`, `PieChart`, `ScatterChart`, `AreaChart` struct fields are undocumented
- `render/pipeline/` — Several internal helper functions lack docs (e.g. `normalized_progress_u32`, `normalized_progress_i32`)
- `theme/manager.rs` — `resolve_style()` method doc