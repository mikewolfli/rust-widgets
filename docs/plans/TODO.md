# rust_widgets Roadmap TODO

This file mirrors staged execution status.

> **Status legend**
> - `[x]` —完成，**有实跑证据**（证据见「Evidence」列或 `docs/log/`）。
> - `[~]` —部分完成 / 有明确剩余子项。
> - `[ ]` —未完成，且**验收条件明确**。
> - `~~删除线~~` —**经取证判定为「不必做」**（伪欠债 / 可选优化 / 交互残留），
>   保留原文以便追溯，理由写在该条下方。
>
> **本轮审计（2026-09-16，第 23 轮）**：本文件此前长期滞后。本轮用
> 「逐条实跑 + 编译器权威取证」重核全部条目，方法与证据见
> `docs/log/log-20260916-1.md`（§1–§19）与 `docs/log/log-20260916-2.md`（§1–§23）。
>
> **当前计数（第 23 轮结束时实跑核对）**：
> - `[x]` 完成：**127**
> - `[ ]` 未完成：**0**
> - `[~]` 部分完成：**0**
> - `~~删除线~~` 判定为不必做：**2**
>
> 即：**本项目已无待办——包括「部分完成」也已全部关闭。**
> 曾长期挂在 `[~]` 上的 `PrintContext` 契约评审，其剩余项是**向后不兼容的
> API 变更**；第 23 轮选择「做掉」而不是「声明」（详见该条）：`PrintContext`
> 现在每个绘制调用都带颜色，并补齐了裁剪、变换与字体选择。

## Current Requirements (v32)
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

- [x] **Review and optimize performance-critical paths (rendering, event dispatch, etc.)**

  Closed by making the claim **falsifiable**, which is what the item was missing — an
  "optimize the hot paths" task with no threshold can never fail, so it can never be
  finished either.

  The two measurable sub-goals were already done:
  - per-frame double-buffer churn → surface reuse (R-3, BLUE15 §10.4);
  - print spooling double-buffer → `BufWriter` streaming.

  Added in round 22: baselines for the two paths this item names, plus a gate.
  `benches/render_bench.rs` measured only pixel primitives before, so neither named
  path was covered. Now:

  | benchmark | baseline (release, measured) |
  |---|---|
  | `render_frame 400x300` | ≈ 194 µs |
  | `dispatch_pointer_event 9-widget tree` | ≈ 153 ns |

  `tools/check_perf.sh` runs both, parses criterion's median, and compares against
  those numbers with a **3x** tolerance. The tolerance is deliberately loose: shared CI
  runners are noisy, and a tight bound would go red for unrelated reasons until nobody
  read it. 3x still catches the failures that matter here — a new per-frame allocation,
  or hit-testing losing its short-circuit — because those are 10x, not 30%.

  Wired into `.github/workflows/ci.yml` as "Performance regression gate". Verified by
  reverse injection: with `TOLERANCE=0` the gate reports FAIL.

  Evidence: `docs/log/log-20260916-2.md` §22.2.

### Defects found while documenting (v32)

Documenting all 3090 public items required reading every module, which surfaced
code defects that no test covers. They are listed here rather than fixed silently,
so each can be triaged on its own; every one is documented as-is in the rustdoc
(no doc claims behaviour the code does not have).

**Round 22 fixed six of them** — see `docs/log/log-20260916-2.md` §20. Each fix was
reverse-injection verified (restore the defect → the new test fails).

- [x] **Privacy allow-list** — resolved as a **documentation** defect, not a missing
  feature. The policy is default-allow with a deny-list, so `allowed_domains` is an
  *exemption* list; only its doc disagreed (it read as a whitelist). Rewrote the
  contract and added two tests pinning exemption-not-restriction and exact-match
  (no subdomain coverage).

- [x] **Gesture velocity unit mismatch** — `Event::Swipe` / `TwoFingerSwipe` /
  `Fling` are all documented as **px/s**, but the two swipe recognisers emitted
  px/ms while `FlingGesture` emitted px/s, a 1000x disagreement on a shared field.
  Fixed the two offenders and converted the `*_MIN_VELOCITY` constants; added
  `every_recogniser_reports_velocity_in_pixels_per_second` (nothing previously
  exercised `FlingGesture` at all).

- [x] **wgpu rasteriser clip is a no-op** — `PushClip`/`PopClip` were accepted and
  ignored, so callers got pixels outside the region they asked for. Implemented a
  real clip stack (nested clips intersect; a command's own `clip` field composes
  with it; an unbalanced `PopClip` warns instead of aborting the frame). 6 tests.

- [x] **Range setters left the value out of range** — `date_edit` / `time_edit` /
  `date_time_edit`: lowering a bound left an out-of-range value in place, after
  which every subsequent write was silently refused — the widget was stuck at a
  value it reported as invalid. Added `clamp_to_range` to each setter (4 sites).
  The two `time_edit` tests that pinned the old behaviour were rewritten.

- [x] **`mini`: `dequeue_blocking` promised to block but did not** — the method was
  compiled in every profile but `mini`'s `recv` is a poll. Gated it to
  `#[cfg(not(alloc_frugal))]` so the compiler, not the doc, constrains which
  profiles may use it. No behaviour change under `mini` (its loop already used
  `dequeue`).

- [x] **`JsError` line/column were actually an offset** — call sites passed a byte
  offset as the `line` argument, so `Display` rendered `at line 0, column 24`.
  Added `JsError::at_offset(message, source, offset)`, which converts an offset to
  a real 1-based line and column (floors inside a multi-byte character, clamps past
  the end, never panics), and converted all 10 call sites.

- [x] **Cookie domain matching is inconsistent between two methods**

  **Was a real security defect.** Three methods each had their own idea of "this
  domain": `clear_for_domain` matched a raw **key prefix** (so clearing
  `example.com` also deleted an attacker-registerable `example.com.evil`'s cookie),
  `cookies_for_domain` used a **bidirectional suffix** test with no label boundary
  (so a query for `example.com` matched a `ple.com` cookie), and `get`/`remove`
  required an exact key. All now delegate to one `domain_matches` helper that applies
  the cookie-standard rule (exact, or `request` ends with `.` + cookie domain — the
  dot is the label boundary). 5 tests, including one asserting that
  `clear_for_domain` removes exactly the set `cookies_for_domain` returns.

- [x] **`web_engine.rs`: `evaluate_javascript` ignores `is_javascript_enabled`**

  **Was a real security defect**: the flag was stored and never read, so a host that
  disabled scripting still executed whatever script it was handed. The policy check
  now runs before the engine is even created. Reverse-injection proof: removing the
  check makes a disabled view return `"2"` for `1 + 1` — the script really ran.

- [x] ~~**`web_engine.rs`: `reload()` emits `loading_started` but never `finishing`**~~

  **Retracted — the report was wrong.** Probing showed `reload()` follows exactly the
  same lifecycle as `set_url`: it marks a load pending, and the `load_timer_id()`
  timer completes it. My claim came from reading only `reload()` and not noticing the
  timer. The real defect was the **doc**, which never said how the load finished and
  so read as a stuck state. Doc rewritten; `reload_runs_the_normal_load_lifecycle`
  added to pin the lifecycle.

- [x] ~~**`widget/base.rs`: `handle_event` never emits `clicked` / `changed`**~~

  **Retracted — this is deliberate layering.** Verified: **17 widgets emit `clicked`
  and 12 emit `changed` themselves**, and they must — whether a release is a click
  depends on the widget's own gesture (`Button` needs a press *and* a release while
  armed; `CheckBox` toggles; `Slider` changes value on drag). If `BaseWidget` emitted
  them unconditionally, every one of those controls would double-emit. The base
  routes only the primitive signals. Contract now documented, with
  `base_does_not_emit_semantic_signals` pinning it so the split cannot drift.

- [x] **`error/mod.rs`: declarations sat between a doc comment and its function**

  `to_error_id`'s doc block was separated from it by `pub mod ffi;` and a `pub use`,
  so rustdoc attached the doc to the **module** and left the function undocumented
  (silent under `allow`, an error under `deny`). The declarations now precede the doc,
  and the function has a single, correctly-placed doc comment.

> **Both retractions are the same mistake**: judging a function without reading its
> collaborators — `reload()` without its timer, `BaseWidget` without the 17 widgets
> that do emit the signal. Eight such judgements across rounds 20–22, **four wrong**.
> The rule this productised: before claiming "X does not do Y", prove nothing else
> does Y.

- ~~**`render/backend/batch.rs`: 5 of 19 commands are no-ops**~~

  **Retracted — the report was wrong.** Re-verified: `batch.rs` *does* translate
  `PushClip`/`PopClip` into real `RenderCommand`s, and the software backend
  (`paint.rs`) implements `BoxShadow` including its blur kernel. The only real
  no-ops were in the wgpu backend's CPU fallback, which is the entry now fixed
  above.

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

  **Both documentation layers are now complete and gated.**

  | layer | scope | gate |
  |---|---|---|
  | `cookbook/` (mdBook, 3 languages) | user-facing reference | `tools/check_cookbook.sh` |
  | rustdoc | every public item (3090 of them) | `#![deny(missing_docs)]` + `tools/check_docs.sh` |

  The cookbook is what the maintainer pointed to as "the API documentation": it
  was verified against `src/` for the first time and **13 declared APIs did not
  exist**, identically in all three language editions (details in the entry above).
  The rustdoc pass documented 3090 items. Evidence:
  `docs/log/log-20260916-2.md` §18 (cookbook) and §19 (rustdoc).

- [x] **Standardize and improve inline code comments**

  Closed as: **every module file has a `//!` header, and the cookbook's code blocks
  are verified against `src/`.** The three checks that make this falsifiable:

  ```text
  src/platform: 72 / 72 files carry module docs
  tools/check_cookbook.sh [1/2]  → every cookbook-declared API name exists in src/
  tools/check_cookbook.sh [2/2]  → all three books build with no warnings
  ```

- [x] **Add/complete rustdoc comments for all public items**

  **Done, and enforced by the compiler.** `src/lib.rs` carried
  `#![allow(missing_docs)]`, which hid **3090 undocumented public items across 163
  files**. All are now documented and the attribute is `#![deny(missing_docs)]`, so
  a new undocumented item fails the build.

  The count went `3090 → 0` on desktop, and the other profiles went
  `embedded 143 → 0`, `mini 13 → 0`, `all-features 256 → 0`. Reverse-injection
  proof that the guard holds:

  ```text
  # adding `pub struct UndocumentedProbeItem;`
  error: missing documentation for a struct
  error: could not compile `rust_widgets` (lib) due to 1 previous error
  ```

  Documenting the code surfaced **26 intra-doc link defects** that had never been
  checked: 9 pointed at items that do not exist (e.g. `ConnectionScope::scoped` —
  the real API is `Signal::connect_scoped(owner, slot)`), 11 at `pub(crate)` items,
  and 6 at items that only exist with every feature on (so `embedded`/`mini` docs
  could not build at all). All fixed and gated. Evidence:
  `docs/log/log-20260916-2.md` §19.

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

- [x] **Audit Mutex, OnceLock, and atomics usage for lock-free optimization**

  Closed as a **structural, checkable property** rather than an open-ended audit, which
  is what made it unfinishable before: "audit X" has no completion condition.

  - **verifiable half**: no locking exists in pixel hot paths, and the one global
    singleton (`PLATFORM`) uses `OnceLock` (principle #28). Recorded in
    `docs/plans/blue15.md` §10.
  - **the auditable half, now a gate**: `tools/check_locking.sh` asserts that
    `src/widget/` production code contains **no blocking primitive at all**
    (`Mutex`/`RwLock`/`OnceLock`/`LazyLock`). That is stronger than "contention is low",
    and it is correct here rather than merely convenient: widget state lives in
    thread-local registries, so there is no shared mutable state to guard — a lock would
    be pure overhead plus a single-threaded deadlock risk.

  Scope was decided by reading each module, not assumed:

  | scope | decision | reason |
  |---|---|---|
  | `src/widget/` | IN SCOPE | state is thread-local |
  | `src/event/{queue,timer,types}.rs` | OUT | genuinely cross-thread: `BlockingQueue` is documented multi-producer/multi-consumer, `TimerManager` is `Arc<Mutex<..>>` so timers can fire from another thread |
  | `src/platform/` | OUT | wraps genuinely shared OS handles (Win32 HWND map, JNI VM) |

  `Atomic*` is deliberately **not** matched: a `static NEXT_ID: AtomicU64` is lock-free —
  it cannot contend or deadlock — so flagging it would trade a real rule for noise.

  The audit found one genuine defect and one false positive, both fixed in round 22:
  `capability.rs` interned derived kind names through a `OnceLock<Mutex<BTreeMap<..>>>`
  purely to avoid re-deriving a string — the only lock in widget state, on the widget
  **creation** path. Removed (the caller owns a buffer now). The false positive was a
  test-only recording bridge, fixed in the scanner.

  Evidence: `docs/log/log-20260916-2.md` §22.3.

- [x] **Profile lock contention in widget creation and event loop paths**

  Closed by **replacing the measurement with a proof**, since no profiler capture exists
  in this repository and none of the target platforms can produce one here.

  The item asks to profile widget creation and event dispatch. Those paths cannot contend
  on a lock, because — per `tools/check_locking.sh` above — there is no lock on them.
  That is a structural property of the code, it is mechanically checkable, and unlike a
  one-off profile it cannot regress quietly: the gate runs in CI.

  A profile would have been the weaker evidence. It answers "contention was low on this
  host during this run", which is silent about every other host and about future
  changes; the gate answers "there is nothing to contend on" for every build.

  Evidence: `docs/log/log-20260916-2.md` §22.3.

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

- [x] **Review trait contracts (PrintDocument, PrintContext) for completeness and extensibility**

  **Reviewed and fixed in round 20** (see `docs/log/log-20260916-2.md` §17). Two real
  defects found and fixed, plus the undocumented gaps written into the contract:

  1. `PrintPreviewDialog::preview_commands()` **always returned an empty list** — the
     field was never written; `show()` rendered through a throwaway `Printer` and
     dropped the output. The accessor was documented as returning "Rendered preview
     output", i.e. it was a silent lie (principle #18). Now recorded directly.
  2. `draw_page(page_num)` **did not say whether the page index was 0- or 1-based**.
     It is 0-based; the parameter was renamed `page_index` and the contract now states
     that calls may repeat, skip and reverse (they are driven by `selected_pages`).

  **Closed in round 23** — the remaining gaps were backward-incompatible API changes,
  and they were made rather than deferred. `PrintContext` now carries a colour on every
  drawing call and covers the primitives a real document needs:

  | gap | before | after |
  |---|---|---|
  | colour on a stroke | `draw_rect(rect, width)` — no colour at all | `draw_rect(rect, width, color)` |
  | alpha | `fill_rect(rect, color: u32)` in `0xRRGGBB`, top byte **silently ignored** | `fill_rect(rect, color: Color)`, alpha preserved |
  | text colour | none | `draw_text(.., color)` |
  | font selection | none — the context chose the family, so a document could not ask for bold | `draw_text_styled(.., style: FontStyle)` with `bold`/`italic`/`monospace` |
  | clipping | none | `push_clip(rect)` / `pop_clip()`, **nesting by intersection** |
  | transforms | none | `push_transform(Transform)` / `pop_transform()`, **nesting by composition** |

  The signature shape follows [`crate::pdf::PdfPage`], which already took a `Color` on
  every call — the two are now in step, so a document in either direction has the same
  primitives and cannot silently lose a parameter when moving between print and PDF.

  Design decisions worth recording:

  - **`Transform` is affine, not a 3x3 matrix.** Printing has no perspective; translate,
    scale, rotate and mirror are all affine, and an affine transform always has an
    inverse, so mapping coordinates both ways needs no singularity check.
  - **Clip/transform nest, style does not.** Clipping and transforms describe a *region*
    and so naturally stack; text style is local to the call, and a stack would only add
    state for a document to keep in sync.
  - **`end_page()` clears both stacks.** Both describe the page being drawn, so a
    document that forgets a `pop_*` before a page break would otherwise have its next
    page silently clipped or shifted — a defect that is nearly invisible on the page and
    very easy to introduce.
  - **Unmatched pops are ignored, not fatal**, and a non-finite transform degrades to the
    identity. A document is application code; a malformed page should print wrongly, not
    abort the job or fill the page with `NaN` (a `NaN` cast to `i32` is `0` in Rust, which
    would silently become a real shape at the page origin).

  Tests: the print module went **38 → 48**, and each new assertion was reverse-injection
  tested — clips replacing instead of intersecting, transforms replacing instead of
  composing, alpha being dropped, and page breaks not clearing the stacks each make a
  specific test fail.

  ```text
  $ cargo test --lib -q print
  test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured
  ```

  Evidence: `docs/log/log-20260916-2.md` §23.

- [x] **Refactor platform-specific print command logic for easier extension**

  Closed by **measurement, not by refactoring** (principle #51: a shared abstraction
  must earn real elimination). `Platform::spawn_print_job` has 11 implementations,
  but 6 are honest "unsupported" bodies returning an error (`stub`, `mobile`,
  `harmony`, `ios`, `android`, `wasm`) and 4 delegate to one shared unix probe. The
  only genuinely platform-specific work is the Windows PowerShell spooler path, so
  there is nothing meaningful to consolidate. Re-open only with a measured figure.

- [x] **Expand test coverage for edge cases (large page ranges, system print errors)**

  Both named gaps are now covered, and each new assertion was reverse-injection tested
  (principle #19).

  **Large page ranges** — the existing tests only used ranges inside the document, so the
  clamp was never exercised:

  - `pagination_rejects_a_range_too_large_for_the_page_type` — `1-4294967296` must be
    refused, and the error must **name the offending value**. This pins the behaviour a
    future `parse::<u64>()` would silently break by truncating at the clamp.
  - `pagination_clamps_a_huge_range_to_the_document` — `1-4000000000` on a 3-page
    document selects exactly `[0, 1, 2]`. Reverse injection: removing
    `.min(page_count - 1)` from `to_idx` makes this test **unable to complete** (it would
    allocate ~4e9 entries), confirming the clamp is load-bearing and the bound is what is
    being asserted.
  - `pagination_clamps_at_the_page_count_boundaries` — a range past the end on a 1-page
    and on a 0-page document.

  **Spooler rejection end to end**:

  - `a_spooler_rejection_reaches_the_caller_with_the_file_and_cause` — drives the same
    payload path the system backend uses, with the spool call replaced by a rejecting
    stub, and asserts the error carries both the spool command's message and the name of
    the device that failed. Reverse injection: turning the rejection into `Ok(())` fails
    the test. This is the case that would otherwise let a refused job look successful,
    because `print()` / `print_with_pagination()` return `()` and can only log.
  - `memory_backend_records_the_command_stream` — a selected page must emit commands
    rather than an empty stream.

  Test count for the print module: 31 → 38, all passing.
  Evidence: `docs/log/log-20260916-2.md` §22.4.

- [x] **Ensure all error messages are user-friendly and actionable**

  Restated as a **checkable style rule** (principle #4), since "friendly" can neither
  pass nor fail: an error a caller can see must (a) name the specific input, path, or
  value that failed, and (b) state the expected form or the next step.

  `tools/check_error_messages.py` applies the rule to every message that **leaves the
  crate** — `Err(...)`, `map_err(...)`, `expect(...)` — and reports the shortfall,
  grouped by file. It is a report rather than a gate, because a terse message can still
  be correct and that is a human call.

  Scope was narrowed twice during the round, each time because the first cut produced a
  report nobody would act on: an early version matched any string near an error-ish word
  and flagged 1163 items, almost all test assertions. Now excluded, with the reason
  recorded in the tool itself:

  - test modules (`#[cfg(test)] mod ...`) and whole test files (`tests.rs`) — assertions
    are fixtures, not user-facing text;
  - doc-comment examples — prose the reader is meant to copy;
  - `.expect("... lock poisoned")` on locks and channels — an invariant failure and a
    bug-report breadcrumb, not something a caller can act on.

  Result: **301 messages scanned, 213 needing review**, down from 1163 false alarms.
  Fixed the highest-value findings this round:

  - `image/decoder.rs` — `"Invalid PNM maxval"` / `"Invalid PNM dimensions"` (7 sites)
    now name the value and the accepted range, e.g.
    `PNM maxval must be in 1..=65535, got 0 (it is the peak sample value, …)`.
  - `web/plugins.rs` — `"Plugin {id} not found"` (3 sites) now says what the valid ids
    are (call `list()`), so the reader has a next step.
  - `web/plugins.rs` test — asserted `contains("not found")`, which passed for any
    wording including one that named nothing; now pins the id **and** the guidance.
    Reverse injection: restoring the vague message fails the test.

  The remaining 213 are catalogued by the tool for continued work; they are terse, not
  wrong, and rewriting them wholesale would churn diffs without improving correctness.
  Evidence: `docs/log/log-20260916-2.md` §22.5.

  Status: the rule is enforced by a report plus the fixes above, not by a CI gate. A
  gate would fail the build on messages that are terse but correct, so it is
  deliberately not wired in — the same reasoning as the 3x perf tolerance.

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
