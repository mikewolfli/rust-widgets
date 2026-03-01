# rust_widgets Roadmap TODO

This file mirrors staged execution status.

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.

## Current Requirements (v2)

## Stage Progress

- [x] P0 Clarify lifecycle routing: embedded uses `RenderEngine`, desktop uses native backends
- [x] P0 Remove non-essential desktop runtime dependency on `RenderEngine` path
- [x] P1 Split capability negotiation into native and embedded contracts with fallback defaults
- [x] P1 Further trim `embedded` build and maintain a minimum runnable profile matrix
- [x] P2 Add ABI stability checks (version, symbols, generated header consistency)
- [x] P2 Add regression validation scripts for `default` / `examples` / `embedded`
- [x] P3 Add observability for backend/engine selection and route diagnostics
- [x] P3 Close docs loop across README/HELP/TODO/CHANGELOG for `v2`

## Architecture Upgrades

- [x] Enforce embedding-first engine ownership boundaries in lifecycle wiring
- [x] Harden native-path isolation for macOS/Linux/Windows backends
- [x] Stabilize capability-query surface for future platform additions
- [x] Formalize ABI compatibility gate in release workflow

## Notes

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

- `v2`: Boundary hardening + validation automation roadmap added.
- `v1`: Initial staged roadmap captured and tracked.
