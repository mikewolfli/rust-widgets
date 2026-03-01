# rust_widgets Roadmap TODO

This file mirrors staged execution status.

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.

## Current Requirements (v1)

## Stage Progress

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

## Architecture Upgrades

- [x] Dual-engine `RenderEngine` abstraction
- [x] Native/Embedded dual implementations
- [x] Object system reflection/property enhancement
- [x] DPI/IME/accessibility and platform capability expansion
- [x] ABI engineering: versioning + header generation

## Notes

- Current platform abstraction is functional, but not yet migrated to the explicit dual-engine architecture (`RenderEngine` + `NativeEngine` + `EmbeddedEngine`).
- Harmony native bridge callback path and typed trigger pipeline are already landed and can be reused when introducing the dual-engine layer.

---

## Version History

- `v1`: Initial staged roadmap captured and tracked.
