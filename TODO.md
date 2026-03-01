# rust_widgets Roadmap TODO

This file mirrors staged execution status.

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.

## Current Requirements (v7)

## Stage Progress

- [x] P0 Close PDF form serialization gap: emit AcroForm/Widget annotations from `PdfPage` form field API (`add_text_field`/`add_checkbox`/`add_button`)
- [x] P1 Close PDF security persistence gap: map `PdfSecurity` into serialized PDF encryption metadata path (or explicit unsupported diagnostics)
- [x] P1 Improve PDF image embedding baseline: avoid payload tiling fallback and add deterministic stream metadata for dimensions/encoding route
- [x] P2 Add focused regression suite for PDF forms/security/image pipeline and roundtrip behavior
- [x] P3 Add matrix refinement pass: reduce false-positive wording signals in completeness report and add per-module allowlist comments

## Architecture Upgrades

- [x] PDF form architecture parity: declarative field API with serialized AcroForm object graph
- [x] PDF security architecture parity: runtime policy to persisted document metadata contract
- [x] PDF image architecture parity: deterministic image stream strategy without synthetic tiling fallback

## Notes

- `v7` is generated from post-v6 completeness audit and tracks remaining functional gaps (PDF-first).
- `v6` remains the finished baseline and is preserved below as history.

## Current Requirements (v6)

## Stage Progress

- [x] P0 Replace software text fallback block-render with real glyph raster/text layout path (baseline Latin + metrics-consistent rendering)
- [x] P0 Upgrade PDF core from minimal placeholder stream model to stable object/content pipeline (text/line/rect/image operators + stronger read/write roundtrip)
- [x] P1 Expand mobile-api from phase-1 baseline (Window/Button) to usable control slice parity (LineEdit/Label/CheckBox/Slider + trigger routing)
- [x] P1 Reduce trait-default no-op dependency in platform abstraction by wiring concrete backend support for IME/accessibility/clipboard/drag-drop across desktop backends
- [x] P2 Upgrade XML declarative instantiation to richer property application (style/text/state/visibility/enabled/tooltip)
- [x] P2 Add XML model binding baseline for declarative Table/Tree data-model wiring
- [x] P2 Implement i18n startup bootstrap hook (`i18n::init`) for deterministic preload/fallback behavior and diagnostics
- [x] P3 Add module-level feature-completeness matrix report script for `src/` (placeholder/fallback/no-op audit as CI artifact)

## Architecture Upgrades

- [x] Text rendering architecture parity: glyph pipeline instead of rectangle fallback paint
- [x] PDF architecture parity: deterministic content stream model + stronger parser/writer contract
- [x] Mobile platform architecture parity: broaden baseline control/state/event surface beyond phase-1
- [x] Platform capability architecture parity: minimize abstract-trait default no-op behaviors in production backends
- [x] Declarative UI architecture parity: XML-driven model binding depth

## Notes

- `v6` is generated from a full `src/` module review focusing on functional completeness gaps (not style/docs).
- `v5` is preserved as completed baseline history and should remain unchanged except retrospective notes.
- ABI compatibility remains a hard constraint unless a new explicit ABI version bump is approved.

---

## Requirement History (v5)

### Stage Progress

- [x] P0 Replace remaining `StubPlatform` dependency in runtime-critical desktop/mobile paths (Windows)
- [x] P0c De-stub runtime-critical path for Windows desktop backend
- [x] P0b De-stub runtime-critical path for Linux desktop backend
- [x] P0a De-stub runtime-critical paths for Harmony desktop and `mobile-api` baseline
- [x] P0 Build framework-grade event loop semantics: nested loop, modal loop, idle/timer priorities, thread-safe post to UI loop
- [x] P1 Upgrade Model/View stack: selection model, editable model contract, delegate/editor lifecycle, data roles, column/row resize
- [x] P1 Upgrade layout engine: size policy, min/max constraints, stretch factors, spacer items, splitter/docking baseline
- [x] P2 Implement real IME/accessibility bridge per platform (not only capability flags)
- [x] P2 Add action/shortcut/command framework (global shortcut map, enable/disable state, menu/toolbar action binding)
- [x] P3 Add clipboard + drag-and-drop cross-platform API and backend adapters
- [x] P3 Expand rendering stack for high-DPI text/metrics, double-buffering, and richer paint primitives
- [x] P3a Land shared backend state-model baseline and wire Windows adapter split
- [x] P3b Wire Linux backend to shared state-model adapter split
- [x] P3c Wire Harmony backend to shared state-model adapter split
- [x] P3d Wire mobile-api backend to shared state-model adapter split
- [x] P3e Wire macOS backend to shared state-model adapter split
- [x] P4a Chart cartesian layout baseline (axis ticks, labels, legend layout)
- [x] P4b Print pagination baseline (page ranges, copy ordering, collation)
- [x] P4c PDF font path embedding baseline (writer API + embedded font stream)
- [x] P4d Print range-spec parser baseline (`"1-3,5,8-6"` style page selection)
- [x] P4e PDF pagination footer baseline (document page numbering stamp)
- [x] P4f Chart legend overflow baseline (label truncation + `+N more` summary)
- [x] P4g PDF footer layout baseline (configurable page-number margins + font size)
- [x] P4h Chart axis tick-density baseline (configurable X/Y tick counts)
- [x] P4i Print page-parity filter baseline (odd/even/all page selection)
- [x] P4j Chart gridline baseline (configurable cartesian grid rendering)
- [x] P4 Strengthen print/pdf/chart to production level (pagination, font embedding/path, axis/legend/layout system)
- [x] P4k Binding endpoint replacement baseline (python/cpp/java status APIs replace reserved placeholders)
- [x] P4l C++ wrapper + Java/JNI skeleton samples and Python package scaffold (`examples/python/pyproject.toml`)
- [x] P4 Complete binding roadmap (Python package + C++ wrapper + Java/JNI skeleton replacing reserved endpoints)
- [x] P5a Behavior matrix harness script (`tools/check_behavior_matrix.sh`) + report (`target/qa/behavior_matrix_report.md`)
- [x] P5b Visual regression harness script (`tools/check_visual_regression.sh`) + deterministic SVG snapshot tests
- [x] P5c Rendering scene/layer baseline (`RenderScene` + ordered `SceneLayer` composition)
- [x] P5d Rendering text-shaping baseline (cluster-aware shaping for combining marks/ZWJ)
- [x] P5e Rendering paint-backend strategy baseline (`PaintBackend` + `SoftwarePaintBackend` composition path)
- [x] P5f Rendering richer paint primitives baseline (`FillCircle`/`DrawCircle` command + software raster path)
- [x] P5g Rendering stroke-width line baseline (`DrawLineStroke` command + software thick-line raster path)
- [x] P5h Rendering stroke-width rectangle baseline (`DrawRectStroke` command + software thick-rect raster path)
- [x] P5i Rendering rounded-rectangle primitives baseline (`FillRoundedRect`/`DrawRoundedRectStroke` + software raster path)
- [x] P5j Rendering anti-aliasing baseline (coverage + alpha blending for circle/rounded-rect edges)
- [x] P5k Rendering anti-aliased line baseline (Wu-style line raster + `DrawLineAA` command path)
- [x] P5l Rendering anti-aliased circle stroke-width baseline (`DrawCircleStroke` + width-aware AA ring coverage)
- [x] P5m Rendering anti-aliased circle fill baseline (`FillCircleAA` + soft-edge fill coverage)
- [x] P5n Rendering anti-aliased thick-line baseline (`DrawLineStrokeAA` + distance-field coverage)
- [x] P5o Rendering anti-aliased rounded-rect stroke-width baseline (`DrawRoundedRectStrokeAA` + high-sample coverage)
- [x] P5p Rendering anti-aliased rounded-rect fill baseline (`FillRoundedRectAA` + high-sample fill coverage)
- [x] P5q Rendering configurable AA sampling baseline (`set_aa_samples_per_axis` for rounded-rect AA paths)
- [x] P5r Extend configurable AA sampling to circle/line paths (`FillCircleAA`/`DrawCircleStroke`/`DrawLineAA`/`DrawLineStrokeAA`)
- [x] P5s Externalize render quality configuration API (`SoftwareRenderConfig` + `apply_render_config`)
- [x] P5t Expose render quality config at backend facade (`PaintBackend` + `SoftwarePaintBackend` passthrough)
- [x] P5u Add scoped scene compose render config override (`compose_with_backend_config` restores backend state)
- [x] P5v Add runnable render quality demo (`demo_render_quality`) and docs entry
- [x] P5w Expose render AA sample config via C ABI and language wrappers (C/C++/Python)
- [x] P5x Align Java JNI skeleton with render AA sample config setters/getters
- [x] P5y Document render AA config API usage in C ABI quickstart/help
- [x] P5z Sync render AA config docs across multilingual help files (zh-CN/zh-TW/fr/ru)
- [x] P6a Complete embedded render engine runtime loop (shared state + target FPS scheduler + wake-up signaling)
- [x] P6b Complete embedded render engine execution/diagnostic surface (frame task queue + resource registry + runtime stats)
- [x] P6c Expose embedded engine controls/stats through C ABI and language wrappers (C/C++/Python/Java)
- [x] P6d Add minimal C ABI embedded engine integration sample (`examples/c_abi_embedded_engine_demo.c`)
- [x] P6e Add Python embedded engine integration sample (`examples/python/demo_embedded_engine.py`)
- [x] P6f Add Java embedded engine integration sample (`examples/java/RustWidgetsEmbeddedEngineDemo.java`)
- [x] P6g Standardize embedded demo output schema across C/Python/Java (`KEY=VALUE`, same field order)
- [x] P6h Add automated embedded demo schema checker (`tools/check_embedded_demo_schema.sh`)
- [x] P6i Integrate embedded demo schema checker into behavior matrix harness (`tools/check_behavior_matrix.sh`)
- [x] P5 Establish cross-platform behavior test matrix and visual/regression harness comparable to mature GUI frameworks

### Architecture Upgrades

- [x] Event-loop architecture parity with mature GUI execution model
- [x] Backend abstraction split: state model vs native-handle adapters (remove mixed stub/native layering)
- [x] Backend state-model baseline extracted (`src/platform/state.rs`) and integrated in Windows backend path
- [x] Backend state-model integrated in Linux backend path
- [x] Backend state-model integrated in Harmony backend path
- [x] Backend state-model integrated in mobile-api backend path
- [x] Backend state-model integrated in macOS backend path
- [x] Model/View architecture parity (roles, delegates, editing pipeline)
- [x] Rendering architecture parity (text shaping, paint backend, scene/layer strategy)
- [x] Rendering scene/layer strategy baseline (`src/render/mod.rs`: `RenderScene`, `SceneLayer`, `RenderCommand`)
- [x] Rendering text-shaping baseline (`src/render/mod.rs`: `shape_text`, cluster-aware metrics)
- [x] Rendering paint-backend strategy baseline (`src/render/mod.rs`: `PaintBackend`, `SoftwarePaintBackend`, `compose_with_backend`)
- [x] Rendering richer paint primitives baseline (`src/render/mod.rs`: `RenderCommand::FillCircle/DrawCircle`, software circle raster)
- [x] Rendering stroke-width line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineStroke`, `draw_line_with_width`)
- [x] Rendering stroke-width rectangle baseline (`src/render/mod.rs`: `RenderCommand::DrawRectStroke`, `draw_rect_with_width`)
- [x] Rendering rounded-rectangle primitives baseline (`src/render/mod.rs`: `RenderCommand::FillRoundedRect/DrawRoundedRectStroke`, software rounded-rect raster)
- [x] Rendering anti-aliasing baseline (`src/render/mod.rs`: coverage sampling + `blend_pixel` for circle/rounded-rect edges)
- [x] Rendering anti-aliased line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineAA`, `draw_line_aa`)
- [x] Rendering anti-aliased circle stroke-width baseline (`src/render/mod.rs`: `RenderCommand::DrawCircleStroke`, `draw_circle_with_width`)
- [x] Rendering anti-aliased circle fill baseline (`src/render/mod.rs`: `RenderCommand::FillCircleAA`, `fill_circle_aa`)
- [x] Rendering anti-aliased thick-line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineStrokeAA`, `draw_line_aa_with_width`)
- [x] Rendering anti-aliased rounded-rect stroke-width baseline (`src/render/mod.rs`: `RenderCommand::DrawRoundedRectStrokeAA`, `draw_rounded_rect_aa_with_width`)
- [x] Rendering anti-aliased rounded-rect fill baseline (`src/render/mod.rs`: `RenderCommand::FillRoundedRectAA`, `fill_rounded_rect_aa`)
- [x] Rendering configurable AA sampling baseline (`src/render/mod.rs`: `SoftwareSurface::set_aa_samples_per_axis`, grid-based rounded-rect AA coverage)
- [x] Rendering configurable AA sampling extended to circle/line (`src/render/mod.rs`: grid-based circle fill/stroke + line stroke coverage)
- [x] Rendering quality configuration API exposed (`src/render/mod.rs`: `SoftwareRenderConfig`, `SoftwareSurface::apply_render_config`)
- [x] Rendering quality config exposed on backend facade (`src/render/mod.rs`: `PaintBackend::apply_render_config`, `SoftwarePaintBackend::apply_render_config`)
- [x] Rendering scoped compose config override (`src/render/mod.rs`: `RenderScene::compose_with_backend_config`, `compose_to_config`)
- [x] Rendering quality demo and docs wiring (`demos/demo_render_quality.rs`, `demos/README.md`, `Cargo.toml` example entry)
- [x] Render AA config exported through bindings (`src/bindings/mod.rs`, `examples/rust_widgets.generated.h`, `examples/cpp/rust_widgets.hpp`, `examples/python/rust_widgets.py`)
- [x] Java JNI wrapper parity for render AA config (`examples/java/RustWidgets.java`, `examples/java/rust_widgets_jni_bridge.c`)
- [x] Render AA config usage documented for integrators (`docs/C_ABI_QUICKSTART.md`, `docs/HELP.en.md`)
- [x] Render AA config docs synchronized in multilingual help (`docs/HELP.zh-CN.md`, `docs/HELP.zh-TW.md`, `docs/HELP.fr.md`, `docs/HELP.ru.md`)
- [x] Embedded render engine runtime completed (`src/render_engine/mod.rs`: shared singleton state, frame scheduler, task submission, runtime stats)
- [x] Embedded engine ABI/wrapper parity completed (`src/bindings/mod.rs`, `examples/rust_widgets.generated.h`, `examples/cpp/rust_widgets.hpp`, `examples/python/rust_widgets.py`, `examples/java/*`)
- [x] Input/accessibility architecture parity (IME, AT bridge, shortcut/action routing)
- [x] QA governance parity (functional + visual + ABI compatibility matrix)

### Notes

- `v5` is a gap-closure roadmap derived from reviewing `rust_widgets` against mature GUI framework capability baselines.
- `v4` delivery is treated as completed foundation work; `v5` focuses on framework-level completeness and parity.
- ABI compatibility remains a hard constraint unless a new explicit ABI version bump is approved.

---

## Requirement History (v4)

### Stage Progress

- [x] P0 Publish `0.0.2` to crates.io (non-dry-run) and verify crates/docs visibility
- [x] P0 Define ABI bump policy and add release gate documentation
- [x] P1 Implement first real foreign-language binding path (Python/cffi)
- [x] P1 Consolidate C header surface to a single source of truth
- [x] P2 Replace Harmony desktop stub-main-path with stronger native integration
- [x] P2 Add minimal independent embedded render loop path
- [x] P3 Start mobile-api phase-1 with one real platform vertical slice
- [x] P3 Add cross-platform behavior consistency tests (menu + typed trigger + capability)

### Architecture Upgrades

- [x] Stable ABI evolution policy and compatibility matrix
- [x] Binding architecture from reserved endpoints to usable adapters
- [x] Backend de-stub roadmap for Harmony/Embedded runtime core
- [x] Test governance for cross-backend behavioral parity

### Notes

- `v4` focuses on moving from hardened scaffolding to concrete runtime/product completeness.
- `v3` release engineering and validation scripts remain mandatory gates.
- ABI compatibility remains a hard constraint unless a new explicit ABI version bump is approved.

---

## Requirement History (v3)

### Stage Progress

- [x] P0 Prepare `0.0.2` release baseline (version bump, changelog cut, publish dry-run)
- [x] P0 Improve crates.io metadata (`rust-version`, repository/docs/homepage, keywords/categories)
- [x] P1 Add CI gate for `tools/check_profiles.sh` and `tools/check_abi.sh`
- [x] P1 Expose profile-aware capability contract query through C ABI
- [x] P2 Add structured runtime diagnostics output (profile/backend/route)
- [x] P2 Add one-command smoke script for `default` and `embedded` demo validation
- [x] P3 Sync v3 release workflow docs across README + HELP multilingual files
- [x] P3 Finalize v3 delivery checklist and handoff template

### Architecture Upgrades

- [x] Release engineering pipeline (build/check/abi/publish-dry-run/publish)
- [x] Profile-aware ABI contract model consolidation
- [x] Runtime diagnostics schema stabilization
- [x] Multi-language documentation sync governance

### Notes

- `v3` focuses on release engineering hardening and public surface quality after the first crates.io publication.
- `v2` architecture boundaries remain valid and are treated as baseline constraints for new work.
- New work should preserve ABI compatibility unless a new explicit ABI version bump is required.

---

## Requirement History (v2)

### Stage Progress

- [x] P0 Clarify lifecycle routing: embedded uses `RenderEngine`, desktop uses native backends
- [x] P0 Remove non-essential desktop runtime dependency on `RenderEngine` path
- [x] P1 Split capability negotiation into native and embedded contracts with fallback defaults
- [x] P1 Further trim `embedded` build and maintain a minimum runnable profile matrix
- [x] P2 Add ABI stability checks (version, symbols, generated header consistency)
- [x] P2 Add regression validation scripts for `default` / `examples` / `embedded`
- [x] P3 Add observability for backend/engine selection and route diagnostics
- [x] P3 Close docs loop across README/HELP/TODO/CHANGELOG for `v2`

### Architecture Upgrades

- [x] Enforce embedding-first engine ownership boundaries in lifecycle wiring
- [x] Harden native-path isolation for macOS/Linux/Windows backends
- [x] Stabilize capability-query surface for future platform additions
- [x] Formalize ABI compatibility gate in release workflow

### Notes

- `v2` focuses on architecture boundary hardening and validation automation, not broad feature expansion.
- Existing dual-engine landing remains valid: render engine abstraction is primarily for embedded paths, while desktop platforms continue to use native backends.
- Harmony native bridge callback path and typed trigger pipeline remain integrated and should stay compatible with the tightened routing model.

---

## Requirement History (v1)

### Stage Progress

- [x] P0 macOS/Linux E2E path
- [x] P1 XML control-tree instantiation
- [x] P1 ID binding and declarative+imperative mixed usage
- [x] P2 Table minimal Model/View
- [x] P2 Tree minimal Model/View
- [x] P2 Expand core C ABI control coverage
- [x] P3 Real print backend
- [x] P3 Real PDF backend
- [x] P3 Real chart backend
- [x] P3 Embedded deep trimming

### Architecture Upgrades

- [x] Dual-engine `RenderEngine` abstraction
- [x] Native/Embedded dual implementations
- [x] Object system reflection/property enhancement
- [x] DPI/IME/accessibility and platform capability expansion
- [x] ABI engineering: versioning + header generation

### Notes

- Dual-engine architecture is already landed (`RenderEngine` + `NativeEngine` + `EmbeddedEngine`); in practice the render engine abstraction is primarily for embedded paths, while desktop platforms continue to use native backends.
- Harmony native bridge callback path and typed trigger pipeline are already integrated and reused by the current engine/lifecycle layering.

---

## Version History

- `v5`: Framework parity gap-closure roadmap added after full code review.
- `v4`: Product completeness roadmap after v3 hardening.
- `v3`: Release engineering + crates.io quality hardening roadmap added.
- `v2`: Boundary hardening + validation automation roadmap added.
- `v1`: Initial staged roadmap captured and tracked.
