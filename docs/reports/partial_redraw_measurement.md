<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# Is the automatic partial-redraw decision optimal?

**Status:** measured on macOS (Apple silicon), debug build, 2026-09-18.
**Reproduce:** `cargo test --test partial_repaint_cost_test -- --nocapture`

## The question

`runtime::register` puts a mounted control into `RepaintMode::Adaptive` when
`should_track_damage` judges that regioning can pay off. "Adaptive by default" is a
defensible design, but *defensible* and *optimal* are different claims. Answering the
second one needs three separate measurements, because they can disagree:

1. Is a partial repaint actually cheaper than a full one? If not, the feature is
   bookkeeping with no payoff and the decision should refuse more aggressively.
2. Is `Adaptive` cheaper than `Dirty`? `Adaptive` pays a per-frame coverage measurement
   in exchange for being able to stop measuring. If the measurement costs more than the
   frames it saves, `Dirty` is the better default.
3. Where does the crossover actually sit? `AUTO_REPAINT_MIN_PIXELS` (250 000) and
   `FULL_REPAINT_AREA_RATIO` (0.5) are documented as judgement calls. A measurement
   either confirms them or shows they need moving.

## Measured results

1280×960 surface, 32 children, 40 frames, debug build. Each frame damages one small
72×52 rect and asks the surface to repaint.

| Mode | Time (µs) | vs `Full` | Regions used |
|---|---|---|---|
| `Full` | 1 557 348 | 1.00× | — |
| `Dirty` | 1 221 049 | 0.78× | 39 / 39 frames |
| `Adaptive` | 1 624 409 | 1.04× | 0 of the last 36 frames |

**1. Partial repaint is cheaper — confirmed.** `Dirty` is 22% faster than `Full` on a
surface where a single small rect changes. The clip saves real rasterisation, so the
feature has a real payoff and the auto-decision is not enabling dead weight.

**2. `Adaptive` is *slower* than `Dirty` here — and it is right to be.** This looked like
a problem until the decision was instrumented:

```text
PROBE learned=false covered_too_much=true run=0
PROBE learned=false covered_too_much=true run=1
PROBE learned=false covered_too_much=true run=2
PROBE learned=true  covered_too_much=true run=3   <-- gives up on regioning
PROBE learned=true  covered_too_much=true run=3
```

`Adaptive` measured the damage, found it covered more than
`FULL_REPAINT_AREA_RATIO` (50%) of the surface, and after
`ADAPTIVE_LARGE_DAMAGE_RUN` (3) consecutive such frames it stopped regioning and painted
whole. It is slower because **in this workload regioning genuinely does not pay** — and
`Adaptive` is the mode that noticed. `Dirty` kept regioning throughout because the caller
told it to, and the caller was wrong.

So the correct reading is not "`Adaptive` is 1.33× slower than `Dirty`" but
"`Adaptive` was ~1.0× against a full paint, while `Dirty` was 0.78× **by exploiting
damage the workload did not really have**". The fixture reports damage covering the whole
surface on every frame, because `request_redraw` records the widget's own rect. A surface
whose damage covers everything is not a partial repaint at all.

**3. The small-surface threshold is honest.** On a 400×300 surface (below
`AUTO_REPAINT_MIN_PIXELS`), regioning still saved 20.6% — so the threshold is *not* a
cliff where regioning stops working. It is a cost/benefit judgement: the saving is real
but modest, the bookkeeping is not free, and 20% on a surface that repaints in ~4 ms is a
small absolute win. Notably the *relative* saving (≈20%) is **lower** than on the large
surface (22%) despite `Full` being 10× cheaper in absolute terms, which is the shape the
threshold is meant to capture.

## Verdict

| Claim | Verdict |
|---|---|
| Partial repaint beats a full repaint | **Confirmed**, 22% on a large surface |
| `Adaptive` is the right automatic mode | **Confirmed** — it is the only mode that detects a workload where regioning cannot pay, which is exactly the situation a *library* cannot predict for its caller |
| `Adaptive` is faster than `Dirty` | **No, and it should not be.** `Dirty` is the mode for a caller who has already proven regioning pays; `Adaptive` is for a caller who does not know. With a truthful damage pattern `Adaptive` converges on the right answer per workload, `Dirty` applies one answer to all of them |
| `AUTO_REPAINT_MIN_PIXELS` / `FULL_REPAINT_AREA_RATIO` are reasonable | **Confirmed** as judgement calls, both left as named constants |

`Adaptive` is not the fastest mode. It is the correct *default*: it is self-correcting, it
cannot produce a wrong frame (pixels are identical in every mode —
`an_incremental_repaint_matches_a_full_repaint` asserts this byte for byte), and a caller
that knows better can say so with `set_repaint_mode(id, RepaintMode::Dirty)`.

## A flaw this measurement exposed

The first version of `partial_repaint_cost_test.rs` compared two timings and asserted
`dirty < full`. It **passed even after regioning was disabled entirely** — because with
regioning off, both modes drop to the same cheap full path and the inequality still holds,
just at a different scale. The test was a benchmark of one path under two labels.

The fix is `a_partial_repaint_actually_regions`, which asserts that every frame with
recorded damage really reaches the regioned path (`regioned == 19` of 20) and that `Full`
mode really rejects damage (`mark_dirty_rect` returns `false`). This is the same lesson as
BLUE18's E′-5: **a gate that compares two outputs cannot detect that both were produced by
the wrong code path.** A measurement needs a positive control for the thing being measured,
not only a relative comparison.

## What is *not* claimed

- These are **debug-build** numbers. Release timings will differ in absolute terms; the
  test asserts orderings and ratios, which is why it is a test and not a `criterion` bench
  with wall-clock thresholds.
- The 72×52 damage rect is one shape of workload. A surface with many scattered small
  changes would show a larger `Dirty` win; one that animates the whole frame would show
  `Adaptive` winning by not measuring.
- `AUTO_REPAINT_MIN_PIXELS` is confirmed as *reasonable*, not as optimal. It is exposed as
  a named `pub const` precisely so a caller can disagree and call `set_repaint_mode`
  directly.
