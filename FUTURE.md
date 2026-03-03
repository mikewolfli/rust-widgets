# rust_widgets Future Work

This file tracks items that are currently not feasible to complete in the present workspace/runtime constraints.

## Rules

1. This file uses hierarchical TODO management, listing each top-level ITEM one by one.
2. Unfinished items are left blank; completed items are marked "Completed" with a timestamp (e.g., Completed 2026-03-02).
3. Deleted or canceled items are marked "Canceled" or "Deleted" with a timestamp.
4. After all todo list are finished, review and check if there are gaps, added them after current version todo list.

## Current Future Requirements (v1)

## ITEM List

- [ ] ITEM 1: Linux full native parity without `gtk-native` feature
  - Constraint: current non-`gtk-native` path is a preview/state loop and does not provide guaranteed visible native widgets.
  - Blocker: requires stable non-GTK native backend strategy (or mandatory GTK runtime dependency policy).
  - Target completion signal: native-interactive behavior parity with `gtk-native` path.

- [ ] ITEM 2: Harmony desktop native window/render/event-loop integration
  - Constraint: current Harmony desktop backend is preview/state-loop oriented.
  - Blocker: missing production Harmony desktop native bridge and event dispatch binding in this repo scope.
  - Target completion signal: real native window lifecycle, input dispatch, and widget host integration.

- [ ] ITEM 3: Android mobile runtime lifecycle + view bridge (desktop-run parity excluded)
  - Constraint: current Android backend is preview/stub when run from desktop flow.
  - Blocker: requires Android app lifecycle integration (`Activity`/view host) and platform packaging pipeline.
  - Target completion signal: end-to-end mobile runtime with native view attachment and event loop ownership.

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

## Notes

- This file captures **currently constrained** work only (not normal in-progress tasks).
- Active implementation work should continue in `TODO.md`; once blocked by platform/runtime dependency, move item here.
- v1 initialized on 2026-03-03.
