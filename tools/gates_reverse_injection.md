# Gates — reverse-injection evidence

> **What this file is.** One line per `tools/check_*.sh`, recording **what was injected** to
> make the gate fail and **what it reported**. A gate that has never been shown to go red is
> indistinguishable from a gate that always passes, so a missing line is treated the same as a
> `NOT-RUN` gate: **unverified**.
>
> **Why it is not in the plans.** It is the gate suite's own record, not a requirement. It lives
> beside `tools/run_all_gates.sh`, which is the runner that consumes the suite, and
> `tools/check_gates_are_worth_running.sh` is what insists every gate has a line here.
>
> **Format.** `| <gate> | <injected> | <reported> |`. The `reported` column quotes the gate's own
> output, because "it failed" is a claim and the quote is the evidence.
>
> **Status column.** `✔` = verified in this round or a recorded earlier round; `—` = not yet
> injected. A `—` is an honest gap, not a pass.

## Designer generator / registry agreement round (2026-09-30)

| gate | injected | reported |
|---|---|---|
| `check_generator_agrees_with_registry.sh` | removed the `btn` alias from `button_capability` in `src/widget/capability/properties.rs` (`aliases: &["pushbutton", "btn"]` → `&["pushbutton"]`) | `generator/registry agreement FAILED:` / `  - [0] the documented alias \`btn\` (for \`button\`) is no longer creatable, so a document spelling the control \`btn\` would generate a program that builds nothing for it` / `  - [0] the documented alias \`btn\` resolves to \`btn\` instead of \`button\`` / `FAIL: the generator and the registry disagree about what can be constructed` |
| `check_event_variants_have_a_producer.sh` (strengthened, 2026-09-30) | renamed every real `Event::KeyRelease` construction site across the six backends **and** the `src/test/harness.rs` helper call | `event variant audit FAILED: these variants have no producer (principle #75):` / `  - Event::KeyRelease` / `FAIL: an Event variant has no producer` |

> Two notes on the injections above, recorded because a reader will otherwise assume they
> were first-try successes.
>
> `check_generator_agrees_with_registry.sh` did **not** fire on its first design. The first probe
> iterated only the aliases *currently registered*, so removing `btn` merely shortened the list and
> everything left passed — the gate stayed green on the very defect it was written for. The probe
> now asserts a **fixed list of documented aliases** (`btn`, `pushbutton`, `text_label`,
> `main_window`, `toggle`), which is what makes an alias that is *removed* detectable.
>
> `check_event_variants_have_a_producer.sh` passed for a long time while `Event::KeyRelease` had no
> backend producer at all. Two holes let it: `src/test/harness.rs` is a **production** module
> (`pub mod test;`) whose synthetic-event builders counted as producers, and a sibling `tests.rs`
> gated by `#[cfg(test)] mod tests;` in its *parent* file carries no `#[cfg(test)]` of its own. Both
> are now excluded, which is what turned the gate red on a variant that had only ever been built by
> test code.

## Version and documentation-consistency round (2026-09-30)

| gate | injected | reported |
|---|---|---|
| `check_version_consistency.sh` | bumped only `Cargo.toml`: `version = "2.8.3"` → `version = "9.9.9"`, leaving every README, cookbook locale, changelog and lockfile at 2.8.3 | `declared version: 9.9.9` / `FAIL README.md names no version, expected 9.9.9` / `FAIL README.zh-CN.md …` / `FAIL cookbook/en/src/README.md …` / `version consistency: 26 problem(s).` |
| `check_capability_flags_match_their_methods.sh` | flipped `ime: false` → `ime: true` in `src/platform/harmony/platform_impl.rs`, which defines no `ime_bridge` | `❌ a capability flag promises a method the backend does not define (1):` / `harmony: src/platform/harmony/platform_impl.rs declares \`ime: true\` but defines none of ('ime_bridge',) — the trait default would answer for it, so the flag promises a method that cannot answer. The flag escapes the process through \`rw_platform_capabilities()\`.` |
| `check_status_docs_name_real_types.sh` | added `\| Window hosting \| ✅ Implemented \| creates a real \`OH_NativeWindow\` for every widget \|` to `src/platform/harmony/status.md` | `❌ a status page claims a native object its backend does not create (1):` / `src/platform/harmony/status.md:48 claims \`OH_NativeWindow\` in a ✅ row, but that identifier appears in none of ['src/platform/harmony'] — the row describes an object this backend does not create` |
| `check_no_native_control_creation.sh` | appended a `_probe_native_control` creating `CreateWindowExW(.., WC_BUTTON, ..)` to `src/platform/windows/canvas.rs` | `❌ a host control creator appears in the widget path (1):` / `src/platform/windows/canvas.rs:637 creates a win32 control (\`WC_BUTTON\`). The library paints every \`WidgetKind\` itself on every backend, so no host control creator may appear in a widget path` |

> Two notes on the injections above, recorded because a reader will otherwise assume they
> were first-try successes.
>
> `check_status_docs_name_real_types.sh` did **not** report on the first two attempts. The
> first named `NSWindow`/`HWND` in a harmony row, and the second put the row in a section the
> scan skips; neither type is in the gate's harmony list. Reading
> `NATIVE_TYPES` in `check_status_docs_name_real_types.py` and injecting a **listed** name
> (`OH_NativeWindow`) is what produced the finding. An injection that does not fire is
> evidence about the injection, not about the gate — the same distinction this file's own
> header draws for `—` (not yet injected).
>
> `check_no_native_control_creation.sh` likewise missed on the first attempt: the probe was
> inserted at `src/platform/windows/types.rs`, where the marker substring did not exist, so
> nothing was injected at all and the gate passed for the honest reason.

## Control-gallery and icon-gate round (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_icons_not_symbol_glyphs.sh` | reverted `app_bar`'s back affordance to the symbol glyph it no longer draws: `crate::widget::draw_icon_at(context, back_rect, text_color, IconName::ArrowLeft);` → `context.draw_text(Point::new(back_x, back_y), "\u{2190}", &Font::default(), text_color, HorizontalAlignment::Left);` in `src/widget/nav_widgets/app_bar.rs` | `finding: src/widget/nav_widgets/app_bar.rs:343: draws U+2190 '←' (Arrows) as text; use an IconName outline (crate::widget::draw_icon_at) instead — no bundled face covers Arrows, so the glyph degrades to an 8x8 bitmap` / `scanned=641 failed=1` |

## BLUE25 — gates added this round (2026-09-27)

| gate | injected | reported |
|---|---|---|
| `check_semantic_state_has_a_consumer.sh` | added `"text_edit"` to the `:error` kind table in `preset_states.rs` (a kind with no `semantic_state`/`resolved_semantic_border` consumer) | `FAIL  a declared \`:error\` kind has no consumer:` / `text_edit: no control reads \`resolved_semantic_border("text_edit", ..)\`` |
| `check_alias_tables_agree.sh` | deleted the `"wizard" => "wizard_dialog"` row from `alias_for_name` in `capability.rs` | `FAIL  \`wizard\` -> \`wizard_dialog\` is in alias_factory_name but not alias_for_name` |

## BLUE25 FONT — gates added this round (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_cjk_shards_cover_the_face.sh` | narrowed the `han` shard's range in `tools/cjk_vector_shards.txt` from `4E00..511F` to `4E00..4FFF`, dropping 288 codepoints the face still declares | `[2/4] the shards partition the fonts-cjk coverage` / `  288 codepoint(s) in the face but in no shard: U+5000, U+5001, U+5002, U+5003, U+5004, U+5005, U+5006, U+5007, U+5008, U+5009` / `  FAIL  the shards do not partition the face` |
| `check_runtime_fonts_is_opt_in.sh` | emptied the feature's implication in `Cargo.toml`: `runtime-fonts = ["text-shaping"]` → `runtime-fonts = []` | `[1/3] \`runtime-fonts\` is declared and implies \`text-shaping\`` / `  FAIL  Cargo.toml does not declare \`runtime-fonts = ["text-shaping"]\`` |
| `check_cookbook.sh` | renamed a real method in one chapter to one the crate does not define: `` `BaseWidget::request_redraw` `` → `` `BaseWidget::request_redraw_renamed_probe` `` in `cookbook/en/src/chapters/performance-quality.md` | `[1/2] every cookbook-declared API name exists in src/` / `finding: cookbook/en/src/chapters/performance-quality.md:623: \`BaseWidget::request_redraw_renamed_probe\` — \`request_redraw_renamed_probe\` is not defined as a method, constant, type, field or variant anywhere in src/ or examples/` / `checked=196 failed=1` |

## BLUE25 C-wave 1 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_event_producers.sh` | renamed the `DoubleTap` construction site in its producing module: `Event::DoubleTap {` → `Event::DoubleTapX {` in `src/gesture/tap.rs` (the variant's event is then produced nowhere in its own recognizer) | `event producer audit FAILED:` / `  - DoubleTap: no production construction site in src/gesture/tap.rs. A recognizer that cannot emit its event is unreachable, not merely untested — see principle #75` / `FAIL: a gesture event has no reachable producer` |
| `check_declarative_path_repaints.sh` | renamed the repaint call on the declarative write path: `request_repaint(` → `request_repaint_disabled_probe(` in `src/view/apply.rs` (the name is now only mentioned, never called) | `src/view/apply.rs: mentions \`request_repaint\` but never calls it, so the repaint contract is documented rather than honoured` / `FAIL: the declarative property-write path does not request a repaint` |
| `check_control_has_tests.sh` | renamed the control's own type everywhere in the file that declares it: `Toast` → `ASTEROID` in `src/widget/special_widgets/toast/single.rs` (the `pub struct` and its `impl Widget` are renamed, so no test constructs `Toast` any more) | `controls with a test that names them: 185 / 186` / `controls with no test mentioning them:` / `  ASTEROID  (src/widget/special_widgets/toast/single.rs)` / `FAIL: a control has no test that builds it` |
| `check_control_route_matrix.sh` | renamed the trait contract method the matrix cross-checks: `fn create_button` → `fn create_button_probe_missing` in `src/control_backend/trait_def/trait_def.rs` | `control route matrix report written to .../control_route_matrix.md` / `trait contract missing expected create methods for: Button` (exit 4) |
| `check_capability_matrix_truthfulness.sh` | made the generator emit a `✅` claim it cannot back: `CELLS = [CUSTOM] * len(PLATFORMS)` → `CELLS = [NATIVE if p == "Windows" else CUSTOM for p in PLATFORMS]` in `tools/generate_platform_capability_matrix.py` (marking every Windows cell native, while `windows/platform_impl.rs` defines no per-widget `create_*` at all) | `❌ Action on Windows: doc=✅ but create_action (via create_button) grade is Missing in ['windows']` … `❌ WizardDialog on Windows: doc=✅ but create_wizard_dialog (via create_wizard_dialog) grade is Missing in ['windows']` / `capability-matrix truthfulness: checked 189 ✅ desktop cells, 188 contradiction(s)` |
| `check_feature_completeness_matrix.sh` | ✅ **REPAIRED 2026-09-28 — was always-pass by construction.** The script only wrapped a generator (`generate_feature_completeness_matrix.py` has no failure branch and returned 0 on any tree), so no production edit could make it red. **Fix:** the generator gained `--assert-allowlist-is-live`, which fails when a `tools/feature_completeness_allowlist.toml` row suppresses a finding its file/module no longer has; the gate now runs it as step [2]. Running the new assertion immediately found **4 stale rows** (`src/pdf/mod.rs` fallback+placeholder, `src/platform/mod.rs` fallback, `src/xml/mod.rs` — a module that no longer exists), all removed. Injection: appended `[files."src/web/mod.rs"] placeholder = 3` to the allowlist | `❌ the feature-completeness allowlist has stale entries — each suppresses a finding its file or module no longer has, so it excuses nothing while reading as if it does:` / `  src/web/mod.rs: allows 3 \`placeholder\` but the file has 0` / `  FAIL  the feature-completeness allowlist excuses findings that are no longer present` (exit 1) |
| `check_declared_tokens_have_consumers.sh` | declared a new colour role nothing reads: added `pub scratch_probe_colour: Color,` inside `pub struct Colors {` in `src/theme/types.rs` (no `.scratch_probe_colour` consumer exists outside the declaration file) | `FAIL: these \`Colors\` roles are declared but never read by production code:` / `  colors.scratch_probe_colour` / `  Either consume it (a control reads it) or add it to ALLOWED in` |
| `check_focus_ring_respects_reason.sh` | removed both ways the file can honour the rule: renamed `visual_focus` → `visual_focus_probe` and `draws_focus_ring` → `draws_focus_ring_probe` throughout `src/widget/base_widgets/button.rs`, so its two `FocusRing::for_control` sites are guarded by neither arm (line 945's ring is guarded by `self.default_button`) | `FAIL: these controls paint a focus ring without asking the focus reason, so a pointer` / `      click would leave a ring under the cursor:` / `src/widget/base_widgets/button.rs:945: constructs a focus ring without asking \`visual_focus()\`/\`draws_focus_ring()\`, so a pointer press would paint one` / `src/widget/base_widgets/button.rs:964: constructs a focus ring without asking \`visual_focus()\`/\`draws_focus_ring()\`, so a pointer press would paint one` / `check_focus_ring_respects_reason: checked=188 failed=2 sites=4` |

## BLUE25 C-wave 2 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_text_model_is_single_sourced.sh` | appended a second `fn is_wide_scalar(..)` (with different ranges) to `src/render/svg/backend.rs`, so a renderer re-implements the wide-scalar table | `finding: the wide-scalar table is defined 2 times (src/render/svg/backend.rs:1378, src/render/text/line.rs:179); exactly one definition is required` / `singletons=3 failed=1` |
| `check_text_origin_is_a_top_edge.sh` | added an `ascent` term to a text origin in `src/widget/media_widgets/video_player.rs`: `(control_bar_height as i32 - time_metrics.height as i32) / 2` → `... / 2 + time_metrics.ascent as i32` | `failed: 1` / `  src/widget/media_widgets/video_player.rs:459` / `      context.draw_text(` / `FAIL: a text origin carries an \`ascent\` term` |
| `check_text_vertically_centred.sh` | replaced a label's text origin with a halved band height in `src/widget/base_widgets/label.rs`: `Point::new(text_x, line.y)` → `Point::new(text_x, rect.y + rect.height as i32 / 2)` | `failed: 1` / `  src/widget/base_widgets/label.rs:222` / `      context.draw_text(` / `FAIL: a text origin is a halved band height` |
| `check_state_source_is_the_base.sh` | declared widget-level interaction state outside `BaseWidget`: added a private `hovered: bool` field to `Spinner` in `src/widget/display_widgets/spinner.rs` | `FAIL: widget-level hover/press/focus state is declared outside BaseWidget:` / `  src/widget/display_widgets/spinner.rs:33: hovered` |
| `check_spacing_is_not_sibling_layout.sh` | read the shared `spacing` field from a control that has no `label_gap` accessor: inserted `let _probe_gap = self.style().spacing.unwrap_or(0);` into `Spinner::draw` in `src/widget/display_widgets/spinner.rs` | `FAIL: these controls read the style's \`spacing\` without confining it to the` / `      indicator-to-text role, so a theme cannot change one gap without the other:` / `src/widget/display_widgets/spinner.rs:272: reads \`Style::spacing\` without a \`label_gap\` accessor, so the field is not confined to the indicator-to-text role` / `check_spacing_is_not_sibling_layout: checked=188 failed=1` |
| `check_view_failures_are_local.sh` | changed the failure sink's element type so failures cannot accumulate: `pub errors: Vec<ViewError>,` → `pub errors: Vec<()>,` in `src/view/apply.rs` | `finding: apply.rs has no \`Vec<ViewError>\` sink: failures cannot be accumulated, so the only alternative left is to return the first one out of the whole apply` / `checks=3 failures_pushed=4 failed=1` |
| `check_view_keys_are_unique.sh` | duplicated a sibling key in one builder chain in `tests/view_data_binding_closure_test.rs`: `Node::new("group_box").key("root")` → `...key("root").key("root")` | `❌ duplicate sibling keys found:` / `   tests/view_data_binding_closure_test.rs:60  build  key='root'` |
| `check_mode_consistency.sh` | made the mode-2 generator emit a fixed wrong widget name in `src/designer/generator.rs`: `Node::new({})", quote(&node.widget)` → `quote("__probe_renamed__")`, so mode 2's tree no longer matches mode 1's | `test both_modes_agree_on_the_control_tree ... FAILED` / `assertion \`left == right\` failed: mode 1 and mode 2 must describe the same controls in the same order.` / `  left: ["window", "label", "button", "slider"]` / ` right: ["__probe_renamed__", …]` / `FAIL: mode 1 and mode 2 disagree about the tree, the properties or the handlers.` |

## BLUE25 C-wave 3 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_capability_feature_gates.sh` | gated a core module on a capability feature it must not be gated by: `#[cfg(all(feature = "desktop", feature = "i18n"))]` prepended to `src/widget/base.rs` | `❌ src/widget/base.rs:1:#[cfg(all(feature = "desktop", feature = "i18n"))]` … `capability feature gates: FAILED (1 finding(s))` |
| `check_changelog_sync.sh` | added a line to only one of the two changelog copies: `<!-- reverse-injection probe -->` prepended to `CHANGELOG.md` | `❌ the two changelog copies have drifted. First differences:` / `1d0` / `< <!-- reverse-injection probe -->` |
| `check_binding_symbol_coverage.sh` | renamed a published symbol in the generated C header: `rw_backend_name` → `rw_backend_name_missing` in `include/rw_generated.h` | `FAIL python (bindings/python/rust_widgets/__init__.py) is missing 1/131 published symbols: rw_backend_name_missing` / `FAIL nodejs (bindings/nodejs/index.js) is missing 1/131 published symbols: rw_backend_name_missing` |
| `check_json_event_route.sh` | cut the link the route check requires: `control_publishes` → `checks_the_registry` in `src/json/loader.rs` | `❌ the JSON event route and the capability table have diverged (1):` / ` src/json/loader.rs no longer calls 'control_publishes', so nothing validates a declared 'events' name against the capability table` / `FAIL: the JSON event route has drifted from the capability event table` |
| `check_lifecycle_hooks_are_not_build_time.sh` | invoked a lifecycle hook from a build/describe path: inserted `fn __probe(node: &Node) { let _ = node.on_mount(1); }` into `src/view/node.rs` | `FAIL: a lifecycle hook is invoked while a tree is described or compared:` / ` src/view/node.rs` |
| `check_mechanism_has_a_consumer.sh` | gave an acknowledged-unconnected abstraction a consumer: `use crate::style::animation::AnimationDriver as _;` prepended to `src/widget/base.rs` | `AnimationDriver: 1 consumer(s), e.g. src/widget/base.rs` … `AnimationDriver is in ACKNOWLEDGED as unconnected (…) but now has a consumer: src/widget/base.rs` / `FAIL: an abstraction is built and nothing calls it` |
| `check_module_reachability.sh` | removed the required consumer-state section: `//! # Reachability` → `//! # ReachabilityXX` in `src/compat.rs` | `Module reachability violations (principle #72):` / `❌ …/src/compat.rs: declares no '# Reachability' section, so its consumer state is unstated` |
| `check_capability_events_are_emitted.sh` | ✅ **REPAIRED 2026-09-28, now genuinely trippable.** The checker's `EVENTS_RE = r"events:\s*&\[(.*?)\]"` could not read the current `events: events_of!("…")` spelling — all 163 constructors use it, `events: &[` appears 0 times — so it parsed **0 pairs** and could never fail (4 injections, incl. renaming all 188 `*_capability()` fns, all exited 0). **Fix:** added `INLINE_EVENTS_RE` + `EVENTS_OF_RE` and a `published_events()` resolver that reads the generated `CONTROL_STARTS`/`EVENT_SCHEMAS` tables; an unresolvable `events_of!` lookup is now a finding, not a silent `continue`. It parses **333 pairs** and immediately found **2 real defects** (`button: canceled`, `time_edit: popup_visibility_changed` — both `pub` + emitted but absent from the census), fixed by adding them to `tools/event_published_census.txt` and regenerating. Injection: removed the emit sites (`self.canceled.emit();`) from `src/widget/base_widgets/button.rs` | `❌ 1 published event(s) have a signal their widget never emits:` / `   button: canceled  (signal \`canceled\`, struct \`Button\`)` / `   The signal exists, so a caller can subscribe and will never be called.` / `FAIL: a published capability event cannot be delivered` (exit 1) |

## BLUE25 C-wave 4 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_font_data_is_opt_in.sh` | enabled font data in the `default` profile: added `"fonts-cjk-bitmap"` to `default` in `Cargo.toml` | `finding: profile 'default' enables font data via 'fonts-cjk-bitmap'` / `fonts=11 payloads=9 findings=1` / `A profile enables font data, or a payload sits outside the gated directory.` |
| `check_font_licenses.sh` | broke the licence record of a generated font table: `//     SHA-256 (decompressed .hex):` → `//     digest (decompressed .hex):` in `src/render/text/cjk_bitmap_data.rs` | `finding: src/render/text/cjk_bitmap_data.rs: no SHA-256 of the upstream source` / `checked=11 failed=1` / `A generated font table ships without a complete licence record.` |
| `check_generated_font_table_integrity.sh` | broke codepoint ordering in the generated table: `0x4E00` → `0x7FFF` in `src/render/text/cjk_bitmap_data.rs` | `finding: src/render/text/cjk_bitmap_data.rs: codepoints not strictly ascending at 257 (0x7FFF then 0x4E01)` / `glyphs=2361 rows=75552 failed=1` |
| `check_glyph_source_is_the_only_glyph_path.sh` | reached a face table from outside the one glyph path: inserted `use font8x8::BASIC_FONTS;` into `src/widget/media_widgets/audio_visualizer.rs` | `finding: src/widget/media_widgets/audio_visualizer.rs:11: reaches a face table outside src/render/text/glyph_source.rs` (×10) / `scanned=655 failed=10` |
| `check_control_feature_visible_in_own_snapshot.sh` | removed a required feature marker from a snapshot: `rgba(155,155,155,1.00)` → `rgba(154,155,155,1.00)` in `snapshots/svg/floating_label.svg` | `floating_label: 2 marker(s) required, 1 present` … `failed: 1` … `missing feature marker: 'fill="rgba(155,155,155,1.00)"'` … `FAIL: a control does not show its own feature in its snapshot` |
| `check_svg_snapshots.sh` | (a) hand-corrupted a byte in `snapshots/svg/floating_label.svg`; (b) added a rogue `snapshots/svg/rogue_probe_control.svg` | (a) `FAIL  the committed snapshots differ from a fresh export (byte-for-byte):` / `124c124` / `< a58978da…9183  snapshots/svg/floating_label.svg` / `> 3e760281…ca4a  snapshots/svg/floating_label.svg`; (b) `FAIL  expected 390 committed SVGs (188 controls x 2 appearances + 6 extra state(s) x 2 + 2 icon sheet(s)), found 391` |
| `check_wire_rules_are_data.sh` | made the compatibility check stop walking the shared table: `for candidate in WIRE_RULES {` → `for candidate in WIRE_RULES.iter().take(0) {` in `src/widget/capability/wire_rules.rs` | `❌ the wire-compatibility rules are not the shared data source (2):` / `compatibility does not iterate WIRE_RULES directly…` / `compatibility truncates its walk over WIRE_RULES…` / `FAIL: the wire-compatibility rules are not a shared data table` |
| `check_generator_reuses_wire_rules.sh` | gave the generator its own compatibility verdict: planted `fn injected_local_verdict() -> WireCompatibility { WireCompatibility::Rejected("local") }` in `src/designer/generator.rs` | `❌ the generator does not reuse the shared wire rules (1):` / `the generator constructs WireCompatibility::Rejected itself, so it has its own compatibility verdict…` / `FAIL: the generator has its own wire-compatibility rules` |

> Two notes from this wave. `check_control_has_tests.sh` is not trippable by *renaming a constructor* alone — the scan also accepts a bare canonical-name string (`"toast"`) anywhere in a test module (e.g. `src/render/surface.rs`), which is arguably the vacuity the gate's own docstring warns about; the injection that does trip it renames the control type itself. `check_feature_completeness_matrix.sh` is additionally always-pass by construction: it has no failure branch (its generator returns 0 on any scanned tree), so the only external injection that trips it is one that breaks the generator, which is what is recorded.

## BLUE25 C-wave 5 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_abi.sh` | renamed a required symbol in the published header: `rw_create_radio_button` → `rw_create_radio_button_X` in `include/rw_generated.h` (the generator then regenerates the true name, so the snapshot no longer matches the on-disk copy) | `ABI header drift detected in include/rw_generated.h: regenerate with tools/generate_c_header.py --output include/rw_generated.h` + unified diff `-uint64_t rw_create_radio_button_X(...)` / `+uint64_t rw_create_radio_button(...)` |
| `check_android_cross.sh` | renamed one shipped ABI in the gate's own target list: `  aarch64-linux-android` → `  aarch64-linux-android_Z` | `unsupported host: these Android targets are not installed, so this gate cannot check them:` / `  - aarch64-linux-android_Z` / `Install them with:` / `  rustup target add aarch64-linux-android_Z` |
| `check_harmony_cross.sh` | renamed the primary triple in the gate: `PRIMARY="aarch64-unknown-linux-ohos"` → `PRIMARY="aarch64-unknown-linux-ohos_Z"` | `HOST-GATED: rust target aarch64-unknown-linux-ohos_Z is not installed` / `  install with: rustup target add aarch64-unknown-linux-ohos_Z` (exit 2 — HOST-GATED, the file's documented contract) |
| `check_ios_cross.sh` | renamed the simulator target in the gate's list: `  aarch64-apple-ios-sim` → `  aarch64-apple-ios-sim_Z` | `unsupported host: these iOS targets are not installed, so this gate cannot check them:` / `  - aarch64-apple-ios-sim_Z` / `Install them with:` / `  rustup target add aarch64-apple-ios-sim_Z` |
| `check_jni_signatures.sh` | added an unbacked Java declaration next to the `// Lifecycle` marker in `bindings/java/RustWidgets.java`: `static native void nativeInjectedProbe();` (58 Java decls vs 57 Rust exports) | `[1/3] Generic Java binding (io.github.rustwidgets.RustWidgets)` / `❌ Java native \`nativeInjectedProbe\` has no Rust export \`Java_io_github_rustwidgets_RustWidgets_nativeInjectedProbe\`` / `JNI signature check: 58 declarations, 1 error(s)` |
| `check_profiles.sh` | made step [7]'s Rust test filter stale: `platform::tests::embedded_profile_selection_state_roundtrip` → `..._nonexistent` in the gate | `[7/9] embedded P4c regression gate` / `  - running: embedded selection-state roundtrip` / `❌ QA case ran zero tests: embedded selection-state roundtrip` / `   (cargo test exits 0 on a filter that matches nothing — fix the filter)` |
| `check_rw_prefix_is_abi_only.sh` | added an `rw_`-prefixed definition outside the ABI: `pub fn rw_injected_probe() {}` in `src/core/mod.rs` | `❌ src/core/mod.rs:68:pub fn rw_injected_probe() {}` / `       the 'rw_' prefix belongs to the C ABI; a Rust-level name should` / `       rely on its module path instead (see this script's header)` / `rw-prefix gate: FAILED (1 finding(s))` |
| `check_binding_symbol_coverage.sh` | renamed a value-kind the binding must name: `RW_VALUE_RECT` → `XW_VALUE_RECT` in `bindings/nodejs/index.js` (a rename, not a comment-out — the scanner matches tokens anywhere, including comments) | `FAIL nodejs (bindings/nodejs/index.js) cannot name 1/8 value kinds: RW_VALUE_RECT — a reader that cannot match a kind returns 'no such property' and leaks the payload it did not free` |
| `check_declared_targets_ship.sh` | pointed a declared target at a non-existent path in `Cargo.toml`: `tools/widget_gallery.rs` → `tools/widget_gallery_missing.rs` | `❌ Cargo.toml declares a target at 'tools/widget_gallery_missing.rs' but no such file exists` … `❌ 'tools/widget_gallery_missing.rs' is a declared Cargo target but is NOT in the published package; add it to the 'include' list in Cargo.toml` / `❌ check_declared_targets_ship: 2 problem(s)` |
| `check_platform_capability_matrix.sh` | appended an extra row to `docs/plans/platform_capability_matrix.md` (coverage stays satisfied; drift from the generator does not): a `**InjectedProbeKind**` row marked ✅ on every platform | `❌ docs/plans/platform_capability_matrix.md is stale — regenerate with: python3 tools/generate_platform_capability_matrix.py --output docs/plans/platform_capability_matrix.md` + diff showing the extra `\| **InjectedProbeKind** \| ✅ …` line / `❌ 1 error(s) found in platform capability matrix.` |
| `check_platform_impl_matrix.sh` | turned a state-backed `create_*` body into an unclassifiable one in `src/platform/ios/platform_impl.rs`: `self.insert_widget(IosHandleKind::Menu, text, x, y, width, height)` → `log::debug!("ios menu created"); 0` | `error: 1 unclassifiable implemented create methods` |

## BLUE25 C-wave 6 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_widget_kind_count.sh` | made a doc disagree with the enum: `180 kinds everywhere` → `181 kinds everywhere` in `README.md` | `WidgetKind variants (parsed from src/widget/kind.rs): 180` / `❌ README.md:17 states '181' but the enum has 180` / `check_widget_kind_count: FAILED (1 mismatch(es))` |
| `check_widget_registration_fidelity.sh` | removed a kind's only constructor mapping in `src/widget/capability.rs`: `WidgetKind::DirectoryDialog => "file_dialog",` → `=> "",` | `Unreachable WidgetKind variants:` / `  ❌ DirectoryDialog (alias of FileDialog, which has no constructor)` / `Every kind must be registered in the factory, declared a pub type alias of a registered kind, or carry a // kind-role: base\|child marker in src/widget/kind.rs.` |
| `check_control_rendering.sh` | made a control paint nothing: early `return;` at the top of `Button::draw` in `src/widget/base_widgets/button.rs` | `[1/4] census: rendering every published control in light and dark` / `  FAIL  control_rendering_census_test` / `test p1_every_control_paints_something_unless_known_invisible ... FAILED` / `test p3_chrome_follows_the_appearance_unless_exempted_as_data ... FAILED` / `these controls painted nothing and are not recorded as known-invisible: ["button"]` |
| `check_declaration_implementation_alignment.sh` | made the checked-in census stale: `controls   ` → `controls  999 ` in `tools/declaration_alignment_census.txt` | `[2/2] census: the counted table is present and consistent with the registry` / `the alignment census is stale: the source and the checked-in table disagree.` / `  FAIL  a declaration does not match its implementation` |
| `check_behavior_matrix.sh` | forced a selected contract to fail: added `assert!(false, "INJECTED PROBE FAILURE");` to `consistency_capability_contract_by_profile` in `src/platform/tests.rs` | `[1/14] default profile capability contract` / `test platform::tests::consistency_capability_contract_by_profile ... FAILED` / `panicked at src/platform/tests.rs:113:5: INJECTED PROBE FAILURE` / `❌ case failed or exceeded 900s: default capability contract` |
| `check_enabled_is_honoured.sh` | removed a control's enabled guard: `is_enabled()` → `is_enabled_probe_x()` in `src/widget/input_widgets/cascader.rs` | `EventHandler impls found: 173` / `  guarded by is_enabled(): 145` / `❌ 1 input-handling control(s) never consult enabled:` / `   Cascader  (src/widget/input_widgets/cascader.rs)` / `FAIL: a control ignores its  state` |
| `check_enabled_is_honoured_containers.sh` | removed an in-file justification for an ungated emitting mutator: deleted `deliberately not gated by \`enabled\`` from `src/widget/display_widgets/arc.rs` | `Passive/container files with ungated emitting mutators: 6` / `❌ 2 programmatic emit(s) ignore enabled in allowlist file(s):` / `   src/widget/display_widgets/arc.rs::set_value() emits 'changed' while the control may be disabled` / `   src/widget/display_widgets/arc.rs::set_range() emits 'changed' while the control may be disabled` / `FAIL: a container emits a programmatic signal while it may be disabled` |
| `check_test_guard_uniqueness.sh` | ✅ see "BLUE25 C-wave 6"; **REPAIRED and re-injected**: broadened `DECLARES_LOCK`/`GUARD_SIG` in `check_test_guard_uniqueness.py` to tolerate path-qualified spells (`std::sync::OnceLock<std::sync::Mutex<()>>`, `crate::compat::MutexGuard`), which the old bare-name regex missed — exactly the modules the defect lives in. Injection: added a `static QUALIFIED_LOCK: std::sync::OnceLock<std::sync::Mutex<()>>` to `embedded_engine.rs`'s `test_guard()` | `These functions declare their own \`OnceLock<Mutex<()>>\` instead of delegating to the canonical \`pub *_test_guard\` …` / `  ❌ src/render_engine/embedded_engine.rs:108: \`fn test_guard()\`` / `Fix: delete the local static and delegate to the canonical guard …` (old regex on the injected line: `False`; new: `True`) |
| `check_visual_regression.sh` | changed a value a snapshot pins: `chart.set_x_tick_count(4)` → `5` in `svg_snapshot_line_chart_stable` (`src/widget/chart_widgets/tests.rs`) | `[*] line chart SVG snapshot stable (widget::chart_widgets::tests::svg_snapshot_line_chart_stable)` / `test widget::chart_widgets::tests::svg_snapshot_line_chart_stable ... FAILED` / `panicked at src/widget/chart_widgets/tests.rs:218:5:` / `❌ line chart SVG snapshot stable FAILED (see above)` / `Visual regression checks FAILED.` |
| `check_view_platform_gate.sh` | ✅ **REPAIRED 2026-09-28, then injected.** The script's heredoc (`cat > "$PROBE" <<'PROBE'` at line 83) had **no closing `PROBE`**, so bash consumed lines 83–237 as heredoc text: `check_profile()` and all six invocations were never defined/run, and the gate exited 0 printing nothing. **Fix:** added the missing `PROBE` terminator (now line 100). It now runs all five steps. Injection: re-spelled `src/lib.rs`'s `#[cfg(declarative_view)] pub mod view;` as a hand-written `#[cfg(all(any(feature="desktop", "tablet", "mobile"), widgets_unstripped))]` conjunction (dropping the opt-out) | `=== [5] The gate must be stated as the single 'declarative_view' alias ===` / `  ❌ src/lib.rs's \`pub mod view\` does not use the \`declarative_view\` alias:` / `       275-#[cfg(all(any(feature = "desktop", …), feature = "widgets_unstripped"))]` / `view platform gate: FAILED (4 finding(s))` (exit 1) — exactly the failure the gate's own footer predicts |

## BLUE25 C-wave 7 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_animation_has_a_driver.sh` | declared an animation `tick` with no `Widget::tick` bridge: prepended `pub fn tick(&mut self, delta_us: u64) -> bool { … }` to `src/widget/base_widgets/label.rs` | `FAIL: these controls declare an animation \`tick\` with no \`Widget::tick\`` / `      bridge, so \`runtime::tick_animations\` cannot advance them:` / `  src/widget/base_widgets/label.rs` / `  The bus is the ONLY frame driver (BLUE23 §3.3): a per-control timer would not` / `  be frame-aligned, and driving from each backend would advance a control twice a` / `  frame. Bridge the control instead.` |
| `check_click_requires_release_inside.sh` | added a production `MouseRelease`→`clicked.emit()` arm with no containment and no `MouseLeave` latch to `src/widget/base_widgets/label.rs` | `FAIL: these controls can emit a click from a release outside themselves. Their release` / `      path has no containment test, no MouseLeave latch-clear, and no exemption:` / `src/widget/base_widgets/label.rs:1: emits \`clicked\` from a release path with no containment, no \`MouseLeave\` latch-clear, and no exemption: …` / `check_click_requires_release_inside: checked=188 failed=1` |
| `check_designer_feature_gate.sh` | dropped `&& designer_opted_in` from the `designer_tooling` cfg condition in `build.rs`, so `tablet`/`mobile` activate the designer | `=== [2/3] every other profile must NOT resolve it ===` / `  LEAK: \`tablet\` resolves \`rust_widgets::designer\`` / `  LEAK: \`mobile\` resolves \`rust_widgets::designer\`` / `  absent on mini (correct)` / `  absent on embedded (correct)` / `FAIL: the designer tooling leaks into a delivery profile.` |
| `check_designer_manifest_roundtrip.sh` | changed the event `keys::NAME` key literal to `"name_injected"` in `write_manifest` (`src/widget/capability/designer_manifest.rs`) | `FAIL: the designer manifest does not round-trip` / `panicked at tests/designer_manifest_roundtrip_test.rs:141:5: \`slider.value_changed\` is the payload sentinel and must be in the document` / `panicked … :172:63: the document parses: ManifestParseError { detail: "unexpected key \`name_injected\` in an event" }` / `panicked … :98:5: only 25 controls were checked, so this proves little` |
| `check_embedded_demo_schema.sh` | added an extra output key to a demo: prepended `printf("INJECTED_EXTRA=1\n");` to `examples/c_abi_embedded_engine_demo.c` | `[FAIL] C demo: output schema mismatch` / `  file: examples/c_abi_embedded_engine_demo.c` / `  expected: ['DEMO_PROFILE', 'ABI_VERSION', …]` / `  found:    ['INJECTED_EXTRA', 'DEMO_PROFILE', …]` / `[OK] Python demo: schema and order match` / `[OK] Java demo: schema and order match` |
| `check_event_model_signal_first.sh` | introduced a wxWidgets-style event table: prepended a string containing `BEGIN_EVENT_TABLE … EVT_BUTTON(…) … END_EVENT_TABLE` to `src/widget/base_widgets/label.rs` | `src/widget/base_widgets/label.rs:1:const _INJECTED_TABLE: &str = "…";` / `❌ Found blocked event-table patterns.` / `Use signal/slot routes (Signal<T>/GenericSignal + connect/emit), not wxWidgets-style tables.` |
| `check_generated_sources.sh` | introduced committed-output drift: inserted `// INJECTED DRIFT BY PROBE` into `examples/generated_project/src/generated/ui_default.rs` | `=== [2/4] regenerating reproduces the committed bytes ===` / `Generated source drift: examples/generated_project/src/generated/ui_default.rs` / `--- …ui_default.rs` / `+++ /tmp/…/ui_default.rs` / `@@ -1,5 +1,4 @@` / `-// INJECTED DRIFT BY PROBE` / `FAIL: … does not match examples/generated_project/project.json.` |
| `check_generator_output_compiles.sh` | made the generator emit an import that does not compile on a stripped profile: added `use rust_widgets::view::Node;` to the stripped template in `src/designer/generator.rs` | `error[E0432]: unresolved import \`rust_widgets::view\`` / `… #[cfg(declarative_view)] … pub mod view;` / `error: could not compile \`rw_generated_probe\` (lib) due to 1 previous error` / `panicked at tests/generator_output_compiles_test.rs:201:5: the generated mini program must compile.` / `FAIL: a generated program does not compile on its target profile.` |
| `check_implicit_size_uses_metrics.sh` | made a size hint derive from literals: prepended `fn size_hint` returning `Size::new(self.text.len() as u32 * 8 + 24, 24)` to `src/widget/base_widgets/label.rs` | `FAIL: these size hints derive an answer from literals instead of the shared metric system,` / `src/widget/base_widgets/label.rs:1: \`size_hint\` derives its answer without ControlMetrics, estimate_text_width/estimate_line_height or dimensions::` / `check_implicit_size_uses_metrics: checked=188 failed=1` |
| `check_readable_flag_is_true_when_get_answers.sh` | flagged a property unreadable while its `get` answers it: flipped the first `message_box` `PropertySchema::new("text", …)` row from `true, true` to `false, true` in `properties_dialog.in.rs` | `failed: 1` / `  message_box: \`text\` is flagged \`readable: false, writable: true\`, so a caller can \`set\` it and \`read_property\` will refuse to read it back while \`get\` in src/widget/dialog/message_box.rs has a "text" => arm` / `FAIL: a property is flagged unreadable while its control's get answers it` |

## BLUE25 C-wave 8 — gates injected (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_apple_thread_safety.sh` | removed the main-thread guard: renamed `objc2::MainThreadMarker::new()` → `new_unchecked()` in `src/platform/macos_objc2/platform_impl.rs` | `❌ src/platform/macos_objc2/platform_impl.rs: expected >=1 MainThreadMarker guard, found 0` / `check_apple_thread_safety: FAILED with 1 issue(s)` |
| `check_empty_is_not_answered.sh`-style note: `check_error_messages.py` | added a vague message: `return Err("operation failed".to_string());` at the top of `ffmpeg_encode` in `src/audio/ffmpeg_encoder.rs`. **This file is a REPORT, not a gate — it always exits 0** (its own docstring says so), so the observable change is a delta in reported output | baseline `Scanned 147 error message(s)… 43 do not satisfy` → injected `Scanned 148 … 44 do not satisfy`, with the new row `src/audio/ffmpeg_encoder.rs (2)` / `228: operation failed` / `      -> does not name the input; does not state the expected form` (exit 0 both runs) |
| `check_locking.sh` | introduced a blocking primitive into widget state: added `use std::sync::Mutex as WidgetMutex;` to `src/widget/a11y_submit.rs` | `FAIL: 1 synchronisation primitive(s) in widget state:` / `  src/widget/a11y_submit.rs:47: use std::sync::Mutex as WidgetMutex;` |
| `check_platform_create_coverage.sh` | declared a factory with no capability: added `fn create_zzzprobe_widget(&self, …, width: u32, height: u32) -> ObjectId;` to `src/platform/types.rs` | `UNRESOLVED (1): ['zzzprobe_widget']` / `zzzprobe_widget              UNRESOLVED` / `check_platform_create_coverage: FAILED` |
| `check_routing_match_is_exhaustive.sh` | added a wildcard arm to the routing match: `_ => ControlRoutePreference::CustomRequired,` in `route_is_library_painted` (`src/control_backend/routing.rs`) | `finding: route_is_library_painted has 1 wildcard arm(s). …` / `matches=1 wildcards=1 findings=1` / `The routing decision no longer fails the build for an unrouted WidgetKind.` |
| `check_single_creation_mechanism.sh` | restored a second creation path: made `route_preference_for_widget_kind` return `NativePreferred` for `WidgetKind::Button` (`src/control_backend/routing.rs`) | `❌ route_preference_for_widget_kind still returns NativePreferred —` / `     that arm is the second creation path (BLUE15 §2.5 / G-4).` / `single-creation-mechanism gate: FAILED (1 finding(s))` |
| `check_text_coverage_claim_matches_features.sh` | removed the script-coverage token from the docs: stripped every `ASCII` token from `src/lib.rs` | `finding: src/lib.rs: does not mention 'ASCII', which names the default script coverage` / `docs=2 features=1 failed=1` |
| `check_theme_fixtures.sh` | drifted a fixture from its preset: `background.r` `240` → `241` in `themes/default.json` | `test tests::fixtures_match_the_built_in_presets ... FAILED` / `assertion left == right failed: themes/default.json: background colour drifted` / `themes/default.json: OUT OF DATE (regenerate with themes/generate.sh)` / `Theme fixture checks FAILED.` |
| `check_web_engine_honest.sh` | hardcoded the degradation answer: replaced the body of `has_real_engine` with a literal `false` (`src/web/web_engine.rs`), defeating the platform query | `[3/4] the widget's degradation is queryable and agrees with the platform` / `  FAIL  has_real_engine does not ask the platform, so it is a hardcoded answer` |
| `check_apple_native.sh` | ⚠️ **NOT TRIPPABLE ON THIS HOST — host-gated by design.** No injection is possible: its criterion is macOS-only | `check_apple_native: unsupported host 'Linux' (Apple native verification requires macOS)` (exit 2). The guard `[[ "$(uname -s)" != "Darwin" ]]` returns before any assertion runs; everything past it is a macOS/iOS-Simulator probe. Recorded as a legitimate host-gated observation, not a fake trip. |

## BLUE25 ICON — gates added this round (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_icon_data_is_opt_in.sh` | added `"icons"` to the `default` feature list | `FAIL  \`icons\` is in the \`default\` feature list` |
| `check_icon_licences.sh` | (a) removed the Material Symbols section from a copy of `NOTICE`; (b) ran `tools/gen_icon_data.py` with no `--license` and with `--license=not-a-real-licence`; (c) hand-edited one `d` in `src/widget/icon_data.rs` | (a) `finding: NOTICE: no section headed 'Material Symbols — SVG path subsets for \`icons\`'` / `checked=31 failed=1` (b) both exit 2 (c) `FAIL: src/widget/icon_data.rs is stale; regenerate with` |
| `check_implicit_size_uses_metrics.sh` | dropped the `src/widget/advanced_widgets/dial.rs Dial` row from `tools/implicit_size_exemptions.txt` | `src/widget/advanced_widgets/dial.rs:268: \`size_hint\` derives its answer without ControlMetrics, estimate_text_width/estimate_line_height or dimensions::` / `checked=188 failed=1` |

## BLUE24 — gates added or verified this round (2026-09-24)

| gate | injected | reported |
|---|---|---|
| `check_elevation_is_not_one_value.sh` | put a `Shadow { x: 0, y: 2, blur: 6, .. }` literal back into `role_base_style` | `FAIL  role_base_style builds a Shadow literal instead of reading a level:` |
| `check_single_frame_driver.sh` | (a) swapped `drive_frame`'s `drain_triggers` back to a bare call in the Linux loop; (b) drove the animation bus from the macOS tick | (a) `FAIL  drain_triggers is called outside the frame driver:` (b) `FAIL  tick_animations is driven from outside the frame driver:` |
| `check_animation_state_is_one_type.sh` | re-added `interaction_target: f32` to `Button` | `FAIL  a control keeps its own copy of an animation target: src/widget/base_widgets/button.rs:86` |
| `check_transition_durations_are_tokens.sh` | inserted `from_millis(250)` into `Button::tick` | `FAIL  ... button.rs:337: \`from_millis(250)\` is a hardcoded duration literal` |
| `check_animation_durations_are_tokens.sh` | three separate injections, each observed: added `PropertyDriver::at_ms(value, ms: u32)`; added `TransitionTempo::Millis(u32)`; replaced `duration_ms`'s body with per-arm literals `100/200/300` | `FAIL` on each, escalating: `animation.rs:1824: PropertyDriver::at_ms is a constructor whose tempo parameter is not MotionSlot` → `animation.rs:1574/1564: TransitionTempo::Millis(u32) carries a payload` + `variants are ['Fast','Normal','Slow','Millis']` → `animation.rs:1582/1587/1588/1589: duration_ms does not call motion_tokens() / returns the literal 100 for Fast` |
| `check_first_value_not_zero.sh` | changed a driver's initial value to the target end (`PropertyDriver::at(1.0, ..)`) | reported the offending construction site |
| `check_semantic_state_is_not_in_the_interaction_chain.sh` | returned `WidgetState::Error` from `LineEdit::widget_state` | `FAIL  a widget_state implementation returns a semantic variant: src/widget/input_widgets/lineedit.rs:590` |
| `check_a11y_has_a_producer.sh` | removed the `submit_mounted` / `submit_unmounted` calls from `runtime::{register,unregister}` | `FAIL  \`runtime::register\` no longer submits a mounted control` |
| `check_breakpoints_are_not_in_draw.sh` | inserted `if rect.width < 600 { return; }` into `Label::draw` | `FAIL  a widget branches on a width against a literal: src/widget/base_widgets/label.rs:152` |
| `check_plan_archive_has_index.sh` | dropped an unindexed `.md` into `docs/plans/archive/` | `FAIL  these archived plans are not named in docs/plans/README.md:` |
| `check_generated_files_have_a_runnable_producer.sh` | changed one coordinate in the committed `src/widget/icon_fallback_data.rs` (`382` → `383`) | `FAIL  src/widget/icon_fallback_data.rs does not match tools/gen_icon_fallback.py` |
| `check_generated_files_have_a_runnable_producer.sh` (2026-09-30, `[4/4]` reverse check) | removed the `src/widget/capability/event_payloads.rs` line from `PAIRS`, so a file whose header says `— **generated**` has no declared producer | `FAIL  src/widget/capability/event_payloads.rs carries a generated-file header but no producer is declared for it` / `exit=1` |
| `check_event_variants_have_a_producer.sh` (new, 2026-09-30) | added a variant with no producer to `Event`: `ReverseInjectionProbe { value: u32 }` | `event variant audit FAILED:` / `  - Event::ReverseInjectionProbe` / `exit=1` |
| `check_composite_assembly_rules.sh` | its own `--inject` mode (runs as part of the gate): removes the layout call, then the factory call, from `split_button` | `✅ reverse injection: removing the layout call from split_button is detected` / `…the factory from split_button…` / `…a one-sided child link…` |
| `check_mousepress_carries_modifiers.sh` | replaced `super::types::map_modifiers(ns_event_modifier_flags(event))` with `0` in the macOS press (`src/platform/macos/canvas.rs`) | `❌ src/platform/macos/canvas.rs: reads no live modifier state (expected a platform helper call)` |
| `check_no_write_only_fields.sh` | declared `probe_write_only_field: u32` on `WidgetStyle` and wrote it **only through a struct literal** (`shadow: None,` region), never read | `❌ write-only fields: 1\n   src/style/primitives.rs: \`probe_write_only_field\` is written but never read (principle #99)` |
| `check_environment_is_single_sourced.sh` | (earlier round) relabelled a device fact as a wall-clock read | `FAIL  wall-clock inference of an appearance` |
| `check_native_redraw_goes_through_the_runtime.sh` | removed the `note_native_redraw(id)` pairing from a macOS event handler (`forward_mouse`) | `FAIL  these files queue more platform redraws than they announce: src/platform/macos/canvas.rs:453 in forward_mouse()` |
| `check_no_anonymous_repaint.sh` | inserted a bare `crate::invalidate_surface(self.base().id());` into `progressbar.rs`'s `draw` | `FAIL  these repaints name no cause: src/widget/display_widgets/progressbar.rs:431 in draw()` |
| `check_conclusion_covers_the_gap_table.sh` | four injections, each observed: (a) removed the `§5 Breakpoint` cell from `blue24.md` §13's dimension table; (b) renamed every `§0B` mention inside §13 away; (c) added the count word `五件事` back to §13's opening sentence; (d) removed the `| 8 面的材质 |` mapping row from §14.2 step 4 | (a) `FAIL  §5 is not named in §13 (no \`Breakpoint\`)…` (b) `FAIL  §13 does not name §0B…` (c) `FAIL  §13 opens a sentence with a count word, which must be kept in sync by hand:` (d) `FAIL  §0B row 8 (…面…的材质没有声明通道…) is not in §14.2 step 4's merge table` |

## Earlier gates (BLUE12–BLUE23)

> The gates below predate this file. Their injections were recorded in the round logs under
> `docs/log/`; each is listed here so the coverage is visible, and the ones not yet re-verified
> in this file's own format are marked `—` rather than claimed. `check_gates_are_worth_running.sh`
> counts the `—` rows and prints them, so the backlog is a number rather than an impression.

| gate | injected | reported |
|---|---|---|
| `check_widget_kind_count.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_view_platform_gate.sh` | ✅ REPAIRED (missing heredoc terminator) then injected — see "BLUE25 C-wave 6" | ✅ `view platform gate: FAILED (4 finding(s))` |
| `check_profiles.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_behavior_matrix.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_android_cross.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_no_blanket_allow.sh` | — (removed 2026-09-28: no such file in `tools/`) | — |
| `check_docs.sh` | — (removed 2026-09-28: no such file in `tools/`) | — |
| `check_locking.sh` | — (now injected — see "BLUE25 C-wave 8") | — |
| `check_error_messages.py` | — (a report, not a gate — see "BLUE25 C-wave 8") | — |
| `check_theme_fixtures.sh` | — (now injected — see "BLUE25 C-wave 8") | — |
| `check_surface_style_is_declared_not_hand_rolled.sh` | appended `fn _probe() -> Color { Color::RED.blend(&Color::WHITE, 0.3) }` to `src/widget/base_widgets/label.rs` | `FAIL  29 files hand-derive a bevel; the ceiling is 28.` then `Offending files not already on the list: src/widget/base_widgets/label.rs` |

> **How to add a line.** Run the gate, inject something that must trip it, run it again, and paste
> the gate's own failure text into the `reported` column. If it does not fail, the gate is either
> too narrow or wrong — both are findings.

> **Full suite coverage.** The rows below list every `check_*.sh` that has not yet been
> injected in this file's own format. Each is an honest `—`, and
> `check_gates_are_worth_running.sh` counts them, so the backlog is a number rather than
> an impression. Replace a `—` by running the gate, injecting something that must trip it,
> and pasting the gate's own failure text.

| gate | injected | reported |
|---|---|---|
| `check_abi.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_drawn_types_are_covered.sh` | — (added 2026-09-28; carries a built-in `--inject=<Type>` self-test — see its header) | — |
| `check_animation_has_a_driver.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_apple_native.sh` | ⚠️ host-gated (macOS-only) — see "BLUE25 C-wave 8" | ⚠️ `unsupported host 'Linux'` (exit 2) |
| `check_apple_thread_safety.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_binding_symbol_coverage.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_capability_events_are_emitted.sh` | ✅ REPAIRED (parser could not read `events_of!`) then injected — see "BLUE25 C-wave 3" | ✅ `1 published event(s) ... never emits` |
| `check_capability_feature_gates.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_capability_matrix_truthfulness.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_changelog_sync.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_click_requires_release_inside.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_control_feature_visible_in_own_snapshot.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_control_has_tests.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_control_rendering.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_control_route_matrix.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_declaration_implementation_alignment.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_declarative_path_repaints.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_declared_targets_ship.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_declared_tokens_have_consumers.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_designer_feature_gate.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_designer_manifest_roundtrip.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_embedded_demo_schema.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_enabled_is_honoured.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_enabled_is_honoured_containers.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_event_model_signal_first.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_event_producers.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_event_payload_types.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_event_signal_dyn.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_feature_completeness_matrix.sh` | ✅ REPAIRED (added a real assertion) then injected — see "BLUE25 C-wave 1" | ✅ `allowlist has stale entries` |
| `check_focus_ring_respects_reason.sh` | ✅ see "BLUE25 C-wave 1" | ✅ |
| `check_font_data_is_opt_in.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_font_licenses.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_generated_font_table_integrity.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_generated_sources.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_generator_output_compiles.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_generator_agrees_with_registry.sh` | ✅ see "Designer generator / registry agreement round" | ✅ |
| `check_generator_reuses_wire_rules.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_glyph_source_is_the_only_glyph_path.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_harmony_cross.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_implicit_size_uses_metrics.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_ios_cross.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_jni_signatures.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_json_event_route.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_lifecycle_hooks_are_not_build_time.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_locking.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_mechanism_has_a_consumer.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_mode_consistency.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_module_reachability.sh` | ✅ see "BLUE25 C-wave 3" | ✅ |
| `check_platform_capability_matrix.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_platform_create_coverage.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_platform_impl_matrix.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_readable_flag_is_true_when_get_answers.sh` | ✅ see "BLUE25 C-wave 7" | ✅ |
| `check_routing_match_is_exhaustive.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_rw_prefix_is_abi_only.sh` | ✅ see "BLUE25 C-wave 5" | ✅ |
| `check_single_creation_mechanism.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_spacing_is_not_sibling_layout.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_state_source_is_the_base.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_svg_snapshots.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
| `check_test_guard_uniqueness.sh` | ✅ see "BLUE25 C-wave 6" (parser also repaired) | ✅ |
| `check_text_coverage_claim_matches_features.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_text_model_is_single_sourced.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_text_origin_is_a_top_edge.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_text_vertically_centred.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_view_failures_are_local.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_view_keys_are_unique.sh` | ✅ see "BLUE25 C-wave 2" | ✅ |
| `check_visual_regression.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_web_engine_honest.sh` | ✅ see "BLUE25 C-wave 8" | ✅ |
| `check_widget_registration_fidelity.sh` | ✅ see "BLUE25 C-wave 6" | ✅ |
| `check_wire_rules_are_data.sh` | ✅ see "BLUE25 C-wave 4" | ✅ |
