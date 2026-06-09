# Module Responsibilities (BLUE11 R9.2)

## control_backend/ vs platform/

### control_backend/ (Control Creation & Routing)
- Routes widget creation requests to the appropriate backend
- `NativeControlBackend` → delegates to platform layer
- `CustomPaintControlBackend` → creates state-only widgets
- `routing.rs` → ControlRoutePreference per WidgetKind

### platform/ (Native OS Integration)
- Platform trait: 50+ methods for native widget lifecycle
- Platform-specific: Win32/Cocoa/GTK/Objc2/Wayland backends
- Clipboard, IME, Accessibility, Drag/Drop

### Current Boundary Issues
- NativeControlBackend is a thin passthrough to Platform trait
- CustomPaintControlBackend duplicates WidgetKind routing
- Suggested: Merge NativeControlBackend into Platform trait directly

## web/ vs render/web/

### widget/web_widgets/ (Widget Layer)
- WebView, WebEngine widget implementations
- Widget+Draw+EventHandler trait impls
- URL loading, navigation, JS bridge

### render/web/ (Rendering)
- Web content rendering pipeline
- HTML/CSS parser? (verify)
- Different abstraction level

### Current Issues
- Document which module is responsible for what
- Ensure no code duplication
