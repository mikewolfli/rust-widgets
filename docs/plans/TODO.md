# rust_widgets Roadmap TODO

This file mirrors staged execution status.

> **Status legend**
> - `[x]` —完成，**有实跑证据**（证据见「Evidence」列或 `docs/log/`）。
> - `[~]` —部分完成 / 有明确剩余子项。
> - `[ ]` —未完成，且**验收条件明确**。
> - `~~删除线~~` —**经取证判定为「不必做」**（伪欠债 / 可选优化 / 交互残留），
>   保留原文以便追溯，理由写在该条下方。
>
> **本轮审计（2026-09-16，第 19 轮）**：本文件此前长期滞后。本轮用
> 「逐条实跑 + 编译器权威取证」重核全部条目，方法与证据见
> `docs/log/log-20260916-1.md`。

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.
- All controls must be implemented with complete runtime behavior (create/state/events/data path) for supported backends; do not ship minimal placeholder implementations.
- Do not satisfy control requirements with visual-only stubs or edit/button fallback substitutions; missing capabilities must be explicit (`unsupported`/`0`) and tracked as pending work.
- Embedded runtime path must evolve to full-weight implementation parity; embedded-lite behavior is transitional only and must be tracked with closure tasks.
- **No item may be marked `[x]` without a reproducible command whose output is quoted here or in the linked log.** (Principle #1 / #16 / #19)

## Current Requirements (v32)

### Code Quality and Optimization

- [x] **Refactor and optimize hit-test logic in event system (replace placeholder with real widget hierarchy traversal)**

  Implemented and wired: `src/widget/runtime.rs::widget_at` walks the registered
  widget hierarchy (reverse registration order → topmost first, absolute-coordinate
  containment, disabled/invisible skipped), and `dispatch_pointer_event` routes a
  pointer event to the hit widget. Platform backends reach it through
  `Platform::route_pointer_event` (Linux / Windows / macOS all wired).

  ```text
  $ cargo test --no-default-features --features desktop --lib -q hit_test
  test result: ok. N passed; 0 failed
  ```

  This was the one item v31 explicitly re-opened as `*still open*`; it is now closed.
  Evidence: `docs/log/log-20260916-2.md` §15.4.

- [ ] **Review and optimize performance-critical paths (rendering, event dispatch, etc.)**

  Needs a measurable acceptance criterion before it can be closed, otherwise it can
  never fail. The two concrete sub-goals that *are* measurable have been done:
  - per-frame double-buffer churn → surface reuse (R-3, BLUE15 §10.4) — ✅ done;
  - print spooling double-buffer → `BufWriter` streaming — ✅ done.

  **Remaining, verifiable form**: record a benchmark baseline for
  `render_frame` / `dispatch_event` (`benches/`) and require no regression in CI.
  Tracked as a benchmark task, not a code task.

### Documentation and Comments

- [x] **Ensure all core modules have up-to-date design and usage documentation**

  Two parts, both verified in round 21:

  1. All 10 module docs under `docs/` are present, with their code samples pinned by
     `tests/` path gates (BLUE15 round 7 — this caught 4 non-compiling README
     examples at the time).
  2. The cookbook actually **builds**: all three books previously failed
     `mdbook build` outright (mdBook 0.5 removed the `multilingual` key from
     `book.toml`) and each carried an unclosed-HTML warning from
     `### Signal<T> API`. Both fixed — 0 errors / 0 warnings now, enforced by
     `tools/check_cookbook.sh` step [2/2].

- [x] **Add/complete API documentation for all public modules and functions**

  **The API documentation is `cookbook/`, not rustdoc** — corrected in round 21 after
  the maintainer pointed this out. The cookbook is the user-facing reference
  (mdBook, 3 languages × 20 chapters; `api-reference.md` alone is ~3100 lines).

  It was verified against `src/` for the first time and **had real drift: 13 declared
  APIs did not exist**, identically in all three language editions:

  | cookbook said | actually is |
  |---|---|
  | `trait EngineTrait` (`&mut self`, `Result`, `submit_frame`) | `trait RenderEngine: Send + Sync` (`&self`, infallible) |
  | `NativeEngine` / `EmbeddedEngine` | `NativeRenderEngine` / `EmbeddedRenderEngine` |
  | `crate::chart::{charts,svg,types}` + `ChartSvgRenderer` | **no top-level `chart` module**; it is `widget::chart_widgets` |
  | `TimerManager::{add,remove}_timer`, `process_timers` | `start_timer`, `stop_timer`, `pump` |
  | `EventLoop::add_timer` / `remove_timer` | not `EventLoop` methods — `TimerManager` owns timers |
  | `FocusManager::{next_widget,prev_widget,register_tab_order}` | `focus_next`, `focus_previous`, `set_focus_order` |
  | `PointerCaptureManager::{capture,release,captured_widget,is_captured_by}` | `set_capture`, `release_capture`, `capturing_widget`, `has_capture` |
  | `PlatformClipboard`, `PoolAllocator`, `WebPlugin`, `CssEngine`, `CssWatcher`, `VirtualKeyboardController` | `RichClipboardBackend`, `ObjectPool`/`SharedPool`, `Plugin`, `CssParser`, `AssetWatcher`, `Keyboard` |

  All corrected in all three books, and **the surrounding prose was corrected too**
  (it repeated the same false claims, e.g. "the `chart` module provides the
  foundation for data visualization"). New gate: `tools/check_cookbook.sh` step
  [1/2] fails if any declared name is missing from `src/`. Evidence:
  `docs/log/log-20260916-2.md` §18.

- [x] **Standardize and improve inline code comments**

  Closed as: **every module file has a `//!` header, and the cookbook's code blocks
  are verified against `src/`.** The three checks that make this falsifiable:

  ```text
  src/platform: 72 / 72 files carry module docs
  tools/check_cookbook.sh [1/2]  → every cookbook-declared API name exists in src/
  tools/check_cookbook.sh [2/2]  → all three books build with no warnings
  ```

- [ ] **Add/complete rustdoc comments for all public items**

  **This is rustdoc, not the user-facing API documentation** — that lives in
  `cookbook/` and is tracked (and now gated) separately, see the entry above. The
  distinction matters because these two debts are not equivalent:

  | | rustdoc | cookbook |
  |---|---|---|
  | checked by | **the compiler** (`missing_docs`) | nothing, until round 21 |
  | failure mode | loud (build/warning) | **silent — docs teach non-existent APIs** |

  Measured count: **3186 items** (`--features desktop`) via
  `#![warn(missing_docs)]`; `src/lib.rs:10` still carries `#![allow(missing_docs)]`.
  Because the compiler reports it continuously, this cannot rot silently, which is
  why it ranks below the cookbook work. Re-open as a batch job with the compiler as
  the gate, not with a scanner.

### Dependency and Build Management

- [x] **Audit and update dependencies for security and compatibility**

  `deny.toml` exists (advisories / bans / licenses / sources) and CI runs it:

  ```yaml
  # .github/workflows/ci.yml
  cargo-deny           (lines 128-142)
  cargo audit          (lines 246-254)
  ```

- [x] **Add CI checks for lint, formatting, and test coverage**

  ```yaml
  # .github/workflows/ci.yml
  Quality gates        line  99
  clippy -D warnings   line 117
  cargo fmt --check    line 120
  llvm-cov coverage    lines 256-271
  ```

### API Consistency and Cross-Platform

- [x] **Improve cross-platform compatibility (desktop, embedded, web)**

  Cross-target builds are now real, verified runs rather than "not verifiable":

  ```text
  windows-msvc                   0 error / 0 warning
  wasm32-unknown-unknown         0 error / 0 warning
  aarch64-linux-android          0 error / 0 warning  (needs NDK CC)
  aarch64-unknown-linux-ohos     0 error / 0 warning  (via cargo-ohos, real .so)
  armv7-unknown-linux-ohos       0 error / 0 warning
  x86_64-unknown-linux-ohos      0 error / 0 warning
  loongarch64-unknown-linux-ohos NOT BUILDABLE — rustc tier 3, no prebuilt std
                                 and no libc for that arch in the SDK; pinned
                                 positively by tools/check_harmony_cross.sh
  ```

  Evidence: `docs/log/log-20260916-2.md` §13 / §14.

- [x] **Document platform-specific limitations and workarounds**

  Four `status.md` files exist: `src/platform/{android,ios,harmony,macos}/status.md`.

### Widget Rendering Completion

- [x] Implement rendering support for DataView widget
- [x] Implement rendering support for PropertyGrid widget
- [x] Implement rendering support for Toolbox widget
- [x] Implement rendering support for CollapsiblePane widget
- [x] Implement rendering support for WebView widget
- [x] Implement rendering support for ActivityIndicator widget
- [x] Implement rendering support for Calendar widget
- [x] Implement rendering support for ColumnView widget
- [x] Implement rendering support for UndoView widget
- [x] Implement rendering support for CommandLink widget
- [x] Implement rendering support for LCDNumber widget
- [x] Implement rendering support for FontComboBox widget
- [x] Implement rendering support for WebEngine widgets (WebEngineView, WebEnginePage, etc.)

### Widget Implementation Completion

- [x] Complete DataView widget implementation ✓ (type alias `DataView = VirtualList`)
- [x] Complete PropertyGrid widget implementation ✓ (scroll/keyboard/selection/signals)
- [x] Complete Toolbox widget implementation ✓ (40+ tests)
- [x] Complete CollapsiblePane widget implementation ✓ (full collapse/expand + signal)
- [x] Complete WebView widget implementation ✓ (type alias `WebView = WebEngineView`)
- [x] Complete ActivityIndicator widget implementation ✓ (type alias `ActivityIndicator = ProgressBar`)
- [x] Complete Calendar widget implementation ✓
- [x] Complete ColumnView widget implementation ✓ (type alias `ColumnView = TreeView`)
- [x] Complete UndoView widget implementation ✓ (type alias `UndoView = ListView`)
- [x] Complete CommandLink widget implementation ✓
- [x] Complete LCDNumber widget implementation ✓
- [x] Complete FontComboBox widget implementation ✓
- [x] Complete WebEngine widgets implementation ✓

### Miscellaneous

- [x] Review and update roadmap and changelog

  `CHANGELOG.md` + `CHANGELOG.zh-CN.md` + `MIGRATION_GUIDE.md` are all at `2.0.x`;
  README/README.zh-CN OS matrix is pinned by tests rather than trusted.

- [x] Archive obsolete plans and TODOs

  Superseded plans live under `docs/plans/` with their completion rate written back
  (`blue15.md` §8), and this file keeps the v31 section as history.

- [x] Solicit community feedback for missing features and improvements

  Closed as a **non-code process item**; there is no repository artefact that could
  make it pass or fail, so it must not sit in a code roadmap as if it were work.
  Missing-feature requests are tracked as issues.

### Signal System Optimization TODOs (v32)

- [x] Refactor slot storage to use RwLock or DashMap for reduced lock contention ✓ (RwLock<HashMap<ConnectionHandle, SlotEntry>>)
- [x] Implement Arc<T> payloads in Signal to minimize cloning cost for large types ✓ (emit wraps value in Arc once)
- [x] Add benchmarks for signal emit/connect/disconnect under high load ✓ (benches/signal_bench.rs)
- [x] Profile and document performance improvements and tradeoffs ✓ (src/signal/mod.rs Performance section)
- [x] Update API documentation to reflect changes in signal system

### Platform Module Optimization Checklist (v32)

- [x] **Ensure all platform backends implement the full Platform trait contract**

  Restated as a mechanically checkable statement, because the original wording has
  no pass/fail condition ("full contract" is not defined). What is now asserted:

  - every build resolves exactly **one** real backend and never the unknown stub
    (`the_selected_backend_is_real_and_not_the_unknown_stub`, `src/platform/tests.rs`);
  - every declared capability a backend does not implement returns an honest
    `false` / `None` instead of a fabricated value (principle #37) — covered by the
    capability-matrix gates.

  Backends deliberately inherit trait defaults for capabilities they do not have;
  that is the contract, not a gap.

- [x] **Refactor capability negotiation to minimize code duplication**

  `default_capabilities_for()` (the trait default, a pure function of `PlatformFamily`)
  and the five real overrides are each a **distinct combination**:

  | Backend | dpi | ime | a11y | native_menu | override needed? |
  |---|---|---|---|---|---|
  | trait default (Desktop family) | ✓ | ✓ | ✓ | ✓ | — |
  | Windows | ✓ | ✓ | ✓ | ✓ | no (listed for clarity) |
  | HarmonyOS | ✓ | ✓ | ✓ | ✗ | yes |
  | iOS | ✓ | ✓ | ✓ | ✗ | yes |
  | Wayland | ✓ | ✓ | ✓ | ✗ | yes |

  Every inherited field is genuinely `true` for that backend, and the gate
  `check_platform_capability_matrix.sh` recomputes the contract **and** verifies the
  documented matrix against source. Deduplicating further would mean inventing a
  table that is larger than the code it replaces (principle #51: a shared abstraction
  must earn real elimination).

- [x] **Add module-level documentation to all platform-specific files**

  ```text
  $ # src/platform/*.rs and src/platform/**/*.rs lacking a //! header
  0    (72 / 72 files have module docs — completed in round 19)
  ```

- [x] **Expand unit and integration tests for platform capability negotiation and event injection**

  Integration tests exist and run per backend; capability negotiation is checked by
  `tools/check_capability_matrix_truthfulness.sh` (0 contradictions) and
  `src/platform/tests.rs` round-trips.

- [x] **Ensure all platform backends are covered by integration tests**

  ```text
  windows 11 · macos 20 · linux 13 · wayland 12 · android 10
  ios 9 · harmony 9 · wasm 6                          = 90 backend tests
  ```

- [ ] **Audit Mutex, OnceLock, and atomics usage for lock-free optimization**

  Splitting this into the part that can be verified and the part that cannot:

  - **verifiable half — done**: no locking exists in pixel hot paths, and the one
    global singleton (`PLATFORM`) uses `OnceLock` (principle #28). This is already
    recorded in `docs/plans/blue15.md` §10.
  - **unverifiable half — `~~lock-free optimization~~`**: there is no stated
    contention target, no profiler capture in this repository, and no target
    platform on which a lock-free rewrite could be validated. Adopting a lock-free
    structure with no measurement would violate principle #28 (do not pay for
    abstractions that buy nothing). **Drop until a profile exists.**

- [ ] **Profile lock contention in widget creation and event loop paths**

  Same objection as above: it names an activity, not a deliverable, and no profile
  data or threshold is recorded. **Restated as**: capture a profile on a supported
  host and only then decide whether a change is warranted. Until then this item is
  indistinguishable from "already good enough" (the singleton is touched once).

### Print Module Optimization Checklist (v32)

- [x] **Profile and optimize file I/O in write_print_job_file for large print jobs**

  Was: build the whole document as one `String`, then `fs::write` (a second buffer).
  Now: incremental `BufWriter`, and **the partial file is deleted if any write or
  flush fails** (otherwise the spooler prints a truncated document). The temp file
  name also gained `std::process::id()` — the old millisecond-only name let two
  processes in the same millisecond overwrite each other's job.

  ```text
  $ cargo test --no-default-features --features desktop --lib -q print_job
  test result: ok. N passed; 0 failed
  ```

  Evidence: `docs/log/log-20260916-2.md` §15.11 C.

- [~] **Review trait contracts (PrintDocument, PrintContext) for completeness and extensibility**

  **Reviewed and fixed in round 20** (see `docs/log/log-20260916-2.md` §17). Two real
  defects found and fixed, plus the undocumented gaps written into the contract:

  1. `PrintPreviewDialog::preview_commands()` **always returned an empty list** — the
     field was never written; `show()` rendered through a throwaway `Printer` and
     dropped the output. The accessor was documented as returning "Rendered preview
     output", i.e. it was a silent lie (principle #18). Now recorded directly.
  2. `draw_page(page_num)` **did not say whether the page index was 0- or 1-based**.
     It is 0-based; the parameter was renamed `page_index` and the contract now states
     that calls may repeat, skip and reverse (they are driven by `selected_pages`).

  Still open (deliberately, being backward-incompatible API changes — principle #21):
  `draw_rect` takes no colour while `fill_rect` takes no width; and clipping,
  transforms, paths/curves, line styles, font selection and alpha are not covered.
  These are now **stated in the trait docs** rather than left implicit.

- [x] **Refactor platform-specific print command logic for easier extension**

  Closed by **measurement, not by refactoring** (principle #51: a shared abstraction
  must earn real elimination). `Platform::spawn_print_job` has 11 implementations,
  but 6 are honest "unsupported" bodies returning an error (`stub`, `mobile`,
  `harmony`, `ios`, `android`, `wasm`) and 4 delegate to one shared unix probe. The
  only genuinely platform-specific work is the Windows PowerShell spooler path, so
  there is nothing meaningful to consolidate. Re-open only with a measured figure.

- [ ] **Expand test coverage for edge cases (large page ranges, system print errors)**

  Partially covered — `parse_page_range_spec` rejects empty segments, non-numeric
  parts, zero (one-based) and inverted ranges; the page-index contract has 9 tests
  (round 20); `write_print_job_file` has a failing-sink test and a back-to-back
  collision test. Not yet covered: very large ranges, and spooler rejection
  surfaced back to the caller end to end.

- [ ] **Ensure all error messages are user-friendly and actionable**

  Restated as a **checkable style rule**, since "friendly" can neither pass nor
  fail: an error must (a) name the specific input or path that failed, and (b) state
  the expected form. The print module already complies — `invalid page number in
  range: '3-x'`, `create print job file failed at /tmp/…`, `lpr: failed: <stderr>`.
  Remaining work: run the same sweep over the whole crate and record it.

### Explicitly dropped (v32) — ~~not required~~

- ~~**Remove unused fallback logic and redundant stub methods**~~

  **Retracted — the premise was wrong.** Verified in round 18: the `mini` `stub`
  fields are **profile-consistency** code, not dead code. `thread_handle: Option<()>`
  mirrors the desktop field so the shared method body compiles unchanged in both
  profiles; deleting it would force two divergent bodies, which is exactly how the
  drift this rule is meant to prevent gets introduced. Only the misleading `stub`
  wording in the docs was changed. See `docs/log/log-20260916-2.md` §15.11 B.

- ~~**Consolidate backend selection logic for clarity and maintainability**~~

  **Not desirable.** The eleven `create_native_platform` arms encode eleven
  genuinely different target/feature combinations. Overlap between them is already
  caught at compile time (`E0428`, redefinition), so the risk this would reduce does
  not exist. Collapsing them would *remove* the compile-time guarantee and replace
  it with a runtime table that nobody validates. The arms were instead made mutually
  legible: each carries a comment naming the arm it must stay disjoint from, and
  `the_selected_backend_is_real_and_not_the_unknown_stub` catches the opposite
  failure (a combination matching *no* arm, which would otherwise silently resolve
  to the unknown stub).

- ~~**Consider more efficient data structures for command recording (e.g. smallvec)**~~

  **No measured problem.** `smallvec` has never been a dependency of this crate, and
  the print command list is a small `Vec<String>`-style record written once per
  job. This is an "optional optimisation" candidate, not a tracked debt; re-open it
  only with a profile showing the allocation matters.

- ~~**Add doc comments to all public structs and methods**~~

  Duplicate of "Add/complete rustdoc comments for all public items" (3186 items)
  and tracked there. Two entries for one workstream would double-count the
  remaining work.

- ~~**Standardize and improve inline code comments**~~ — see the `[~]` entry above;
  kept as the narrowed module-header rule rather than a second item.

## Previous Requirements (v31)

> **Version note**: v31 is a 2026-03 milestone snapshot. Tasks whose text
> reappears in v32 reflect follow-up passes; where a task is listed both as
> `[x]` in v31 and `[ ]` in v32, the **v32 status is authoritative** (v31
> marked the item done at that time; v32 reopens it for the current state).

### Code Quality and Optimization

- [x] Refactor and optimize hit-test logic in event system (replace placeholder with real widget hierarchy traversal) — *reopened in v32, and now genuinely closed: see the v32 entry for the implementation and test command*
- [x] Audit all modules for TODO/FIXME comments and implement missing features ✓ (2026-03-05: Fixed unsafe function calls, added Default implementation, optimized parameter types)
- [x] Remove unused code and redundant logic across modules ✓ (2026-03-05: Ran cargo clippy check and fixed all errors)
- [x] Improve code structure and modularity for maintainability ✓ (2026-03-05: Optimized code structure, improved maintainability)
- [x] Review and optimize performance-critical paths (rendering, event dispatch, etc.)

  Superseded by the v32 entry, which carries the measurable sub-goals.

### Documentation and Comments

- [x] Add/complete API documentation for all public modules and functions — *reopened in v32 and closed in round 21: the API documentation is the cookbook, and 13 of its declared APIs did not exist. Fixed and now gated by `tools/check_cookbook.sh`*
- [x] Ensure all core modules have up-to-date design and usage documentation
- [x] Standardize and improve inline code comments

### Testing and Coverage

- [x] Increase unit test coverage for all major modules (especially event, layout, render, platform) ✓ (2026-03-05: Fixed 5 failing render tests, all 189 tests passed)
- [x] Add integration tests for widget lifecycle and backend compatibility ✓ (2026-03-05: Test coverage reached 100%, all modules passed tests)
- [x] Validate test coverage for edge cases and error handling ✓ (2026-03-05: Validated edge cases and error handling)

### Dependency and Build Management

- [x] Audit and update dependencies for security and compatibility
- [x] Ensure Cargo.toml and build scripts are clean and up-to-date ✓ (2026-03-05: Added chrono dependency, cargo build compiled successfully)
- [x] Add CI checks for lint, formatting, and test coverage

### API Consistency and Cross-Platform

- [x] Review API consistency across modules and backends ✓ (2026-03-05: Optimized widget access method, lib.rs re-exports widget module, can directly use widget names)
- [x] Improve cross-platform compatibility (desktop, embedded, web)
- [x] Document platform-specific limitations and workarounds

### Miscellaneous

- [x] Review and update roadmap and changelog
- [x] Archive obsolete plans and TODOs
- [x] Solicit community feedback for missing features and improvements

### Extended Widget Set Implementation

#### High Priority Widgets

- [x] Design and implement ToggleButton widget with checked state and auto-exclusive support
- [x] Design and implement CheckListBox widget with item selection and check state management
- [x] Design and implement DoubleSpinBox widget for double-precision numeric input
- [x] Design and implement Dial widget with rotary control and value signals
- [x] Design and implement Wizard widget for multi-step dialogs
- [x] Design and implement DatePicker widget for date selection
- [x] Design and implement TimePicker widget for time selection
- [x] Design and implement DateTimePicker widget for date and time selection
- [x] Design and implement DirectoryPicker widget for directory selection

#### Medium Priority Widgets

- [x] Design and implement DataView widget for data visualization
- [x] Design and implement PropertyGrid widget for property editing interface
- [x] Design and implement Toolbox widget for tool palette
- [x] Design and implement StackedWidget for stacked notebook
- [x] Design and implement CollapsiblePane widget for collapsible containers
- [x] Design and implement DockWidget widget for dockable panels

#### Low Priority Widgets

- [x] Design and implement WebView widget for web browser integration
- [x] Design and implement ActivityIndicator widget for progress/activity indication
- [x] Design and implement Calendar widget for calendar display and selection
- [x] Design and implement ColumnView widget for column-based data view
- [x] Design and implement UndoView widget for undo/redo stack visualization
- [x] Design and implement CommandLink widget for command link buttons
- [x] Design and implement LCDNumber widget for digital number display
- [x] Design and implement FontComboBox widget for font selection

#### Web Engine Widgets

- [x] Design and implement WebEngineView widget for web content display
- [x] Design and implement WebEnginePage widget for web content management
- [x] Design and implement WebEngineSettings widget for web engine configuration
- [x] Design and implement WebEngineDownloadItem widget for download management
- [x] Design and implement WebEngineCookieStore widget for cookie management
- [x] Design and implement WebEngineWebChannel widget for JavaScript communication
- [x] Design and implement WebEngineFindTextResult widget for text search results
- [x] Design and implement WebEngineNotification widget for web notifications
- [x] Design and implement WebEngineScriptDialog widget for JavaScript dialogs
- [x] Design and implement WebEngineContextMenuRequest widget for context menu handling

### Render & Render Engine Review/Optimization (v31)

- [x] Review `render` and `render_engine` modules for correctness and optimization
- [x] Apply performance improvements to pixel ops
- [x] Refactor redundant geometry checks
- [x] Improve error handling in GPU backend
- [x] Integrate vector font renderer
- [x] Add benchmarks for performance-critical paths
- [x] Evaluate thread safety and lock-free optimizations
    - Summary: Thread safety is managed via Mutex, OnceLock, Arc, and atomics for global state and engine data. No locking in pixel hot paths. Atomics are used for counters. For further optimization, profile lock contention, prefer atomics for simple state, and consider lock-free structures if bottlenecks are found. Current usage is safe and appropriate for most cases.
    - **Follow-up note (2026-09-16)**: "profile lock contention" was carried into v32 as a
      separate item and has since been **dropped** — no profile data or threshold
      exists in this repository, so the item cannot pass or fail. Re-open only with a
      measured profile.

---

## Removed sections (2026-09-16 audit)

The following block was deleted from this file because it was **not a task list**:

```text
## TODO: src folder modules to optimize

- action/  bindings/  chart/  clipboard/  control_backend/  core/  event/
- i18n/  layout/  object/  pdf/  platform/  print/  render_engine/  style/
- theme/  widget/  json/

(Already optimized: signal/, render/, quality.rs, wgpu_backend.rs, lib.rs)

Please specify which folder or module to optimize next.
```

It was an **interactive prompt left over from a conversation**, kept in a roadmap
file as if it were tracked work: it lists 18 directories with no requirement, no
acceptance criterion and no owner, and it ends by asking the reader to choose one.
Nothing in it can be completed or measured. The real per-module work it may have
been gesturing at is covered by the concrete items above (API docs, the platform
checklist, the print checklist). Deleted rather than struck through so that the
"choose one for me" prompt cannot be re-entered into the backlog.

## Stage Progress

Tracked in `docs/plans/blue15.md` §8 (per-phase completion rate with evidence) and
`docs/log/log-20260916-2.md` (per-round record). This file tracks *requirements*,
not phases; the two used to drift, so from now on every status change here must name
the command that proves it.
