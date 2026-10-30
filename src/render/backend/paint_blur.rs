// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The box blurs the software backend runs for shadows, `Blur` and backdrop blurs.
//!
//! Split out of [`super::paint`] so that file stays inside the 2000-line budget
//! (principle #9); the seam is "the kernel" versus "the command dispatch". The
//! two entry points the dispatcher calls are [`blur_rect_on_surface`] (after a
//! shadow has been filled) and [`box_blur_region`] (over a caller-owned buffer).
use crate::core::Rect;
use crate::render::backend::surface::SoftwareSurface;

/// Runs the box blur the shadow commands share over `region` (G-03/G-016).
///
/// Extracted from the `BoxShadow` arm so [`crate::render::core::RenderCommand::BoxShadow`] and
/// [`crate::render::core::RenderCommand::BoxShadowRounded`] cannot drift on how the blur region is
/// clipped: both fill their own source shape and then hand it to this one
/// routine, which intersects the region with the active clip and the surface
/// bounds exactly as the inline block always did.
pub(super) fn blur_rect_on_surface(surface: &mut SoftwareSurface, region: Rect, blur_radius: u32) {
    let size = surface.size();
    let w = size.width as usize;
    let h = size.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    let radius = blur_radius.min(100) as usize;
    let clip = surface.current_clip().unwrap_or((0, 0, w as u32, h as u32));
    let blur_x0 = region.x.max(clip.0).max(0) as usize;
    let blur_y0 = region.y.max(clip.1).max(0) as usize;
    let clip_right = clip.0 as i64 + i64::from(clip.2);
    let clip_bottom = clip.1 as i64 + i64::from(clip.3);
    let blur_right =
        (region.x as i64 + i64::from(region.width)).min(clip_right).min(w as i64).max(0);
    let blur_bottom =
        (region.y as i64 + i64::from(region.height)).min(clip_bottom).min(h as i64).max(0);
    let blur_w = (blur_right - blur_x0 as i64).max(0) as usize;
    let blur_h = (blur_bottom - blur_y0 as i64).max(0) as usize;
    if blur_w > 0 && blur_h > 0 {
        let mut scratch = surface.take_blur_scratch();
        {
            let back = surface.buffer.back_mut();
            box_blur_region(back, w, h, (blur_x0, blur_y0, blur_w, blur_h), radius, &mut scratch);
        }
        surface.return_blur_scratch(scratch);
    }
}

/// Apply a **Gaussian-approximating** blur to a rectangular region of an RGBA pixel buffer.
///
/// # Why a box blur is not what the command asks for
///
/// A `BoxShadow` declares a `blur_radius`, and the only blur a Gaussian has is a *standard
/// deviation*. One pass of a box blur is not that: it produces a flat plateau with hard corners, so
/// a shadow reads as a rectangle with a halo rather than as a falloff — the wider the radius, the
/// more visible the plateau. The crate's snapshots never showed it because no control drew a shadow
/// until the surface channel was wired (`render::surface`), which is exactly when this had to be
/// fixed: a shadow that is *drawn* is a shadow whose quality is visible.
///
/// # The approximation, and why it is the right one here
///
/// Three successive box blurs approximate a Gaussian to within a few percent (the central limit
/// theorem, applied deliberately). Ivan Kutskir's box-width formulas give the three widths whose
/// iterated result has the requested standard deviation, and they are computed for the **first**
/// box so that all three differ by at most one pixel — which is what keeps the result symmetric.
///
/// # Why this is also **faster**, not a cost paid for quality
///
/// The naive implementation this replaces summed `2r + 1` pixels per output pixel per pass, i.e.
/// `O(w * h * r)`, and it re-derived each sum from scratch. A box blur is a convolution with a
/// constant kernel, so the sum is a **sliding window**: leave one pixel, enter one pixel. That makes
/// each pass `O(w * h)` *regardless of radius* across all three boxes — so the Gaussian is not
/// merely comparable to the box blur it replaces, it is asymptotically better and the cost does not
/// grow with the blur the caller asks for.
///
/// # Edge handling
///
/// The region is expanded by the total kernel reach in each direction before the passes run, so an
/// edge pixel is blurred against its neighbours rather than against a shortened window (which would
/// darken or lighten the border). The window itself clamps its indices for the outermost pixels, so
/// the expansion only affects *which* pixels are written, never the arithmetic.
pub(super) fn box_blur_region(
    back: &mut [u8],
    w: usize,
    h: usize,
    region: (usize, usize, usize, usize),
    radius: usize,
    scratch_buffers: &mut super::surface::BoxBlurScratch,
) {
    let (region_x, region_y, region_w, region_h) = region;
    if w == 0 || h == 0 || region_w == 0 || region_h == 0 || radius == 0 {
        return;
    }
    // The three box widths whose iteration approximates `sigma = radius / 2`.
    //
    // The divisor is the convention a Gaussian blur's radius follows: `radius` is the point at
    // which the kernel has decayed to roughly `1 / e^2` of its peak, which is about `2 sigma`. Using
    // `radius` directly as a standard deviation would make every existing shadow roughly four times
    // too soft, so the caller's unit is preserved and only the *shape* is corrected.
    let sigma = (radius as f32) / 2.0;
    let (b0, b1, b2) = box_blur_widths(sigma);
    let reach = b0.div_ceil(2) + b1.div_ceil(2) + b2.div_ceil(2);

    // Expand region by the kernel's total reach in all directions (clamped to surface bounds).
    let ex0 = region_x.saturating_sub(reach);
    let ey0 = region_y.saturating_sub(reach);
    let ex1 = (region_x + region_w + reach).min(w);
    let ey1 = (region_y + region_h + reach).min(h);
    let ew = ex1 - ex0;
    let eh = ey1 - ey0;
    if ew == 0 || eh == 0 {
        return;
    }

    // One scratch row and one scratch column, reused across all three passes.
    //
    // A sliding window does not need a copy of the region — it needs somewhere to stage the values
    // a horizontal pass is about to overwrite while the pass is still reading them (one row, and one
    // column for the vertical pass). These buffers live on the surface and are taken/returned by the
    // caller, so they grow to the largest region once and then cost no allocation at all (G-07).
    let row_len = ew * 4;
    let col_len = eh * 4;
    let copy_len = ew.max(eh) * 4;
    scratch_buffers.row.resize(row_len, 0);
    scratch_buffers.col.resize(col_len, 0);
    scratch_buffers.copy.resize(copy_len, 0);
    let row = &mut scratch_buffers.row[..row_len];
    let col = &mut scratch_buffers.col[..col_len];
    // The untouched row/column being blurred, kept because a pass writes into the same buffer it
    // slides its window over (see `blur_row`).
    let copy = &mut scratch_buffers.copy[..copy_len];

    let stride = w * 4;
    for width in [b0, b1, b2] {
        if width < 2 {
            continue;
        }
        // Horizontal pass: stage each row, blur it, write it back.
        for y in ey0..ey1 {
            let base = y * stride + ex0 * 4;
            row.copy_from_slice(&back[base..base + row_len]);
            copy[..row_len].copy_from_slice(row);
            blur_row(row, ew, width, &copy[..row_len]);
            back[base..base + row_len].copy_from_slice(row);
        }
        // Vertical pass: stage each column, blur it, write it back.
        for x in ex0..ex1 {
            for y in 0..eh {
                let si = (ey0 + y) * stride + x * 4;
                col[y * 4..y * 4 + 4].copy_from_slice(&back[si..si + 4]);
            }
            copy[..col_len].copy_from_slice(&col[..col_len]);
            blur_row(col, eh, width, &copy[..col_len]);
            for y in 0..eh {
                let di = (ey0 + y) * stride + x * 4;
                back[di..di + 4].copy_from_slice(&col[y * 4..y * 4 + 4]);
            }
        }
    }
}

/// The three box widths whose successive application approximates a Gaussian of `sigma`.
///
/// Ivan Kutskir's derivation, which is the standard one for this approximation. Three boxes of
/// widths `wl`, `wl`, `wu` give a variance of `(2 wl^2 + wu^2 - 3) / 12`, so solving for the `wl`
/// that lands nearest the requested `sigma` and taking the next odd width up as `wu` is the whole
/// of it. The two differ by exactly two, which keeps the iterated kernel symmetric — a set like
/// `(2, 5, 2)` would have a different falloff on each side.
///
/// # The per-pass share, which this used to omit
///
/// Variances **add** across independent passes, so the three boxes together must carry `sigma^2`,
/// i.e. each one carries `sigma^2 / 3` and therefore has a width that solves
/// `(w^2 - 1) / 12 = sigma^2 / 3`, i.e. `w = sqrt(12 * (sigma / sqrt(3))^2 + 1)`.
///
/// The width was computed as `sqrt(12 * sigma^2 + 1)` instead — the *whole* variance put into each
/// of the three passes — so the kernel came out `sqrt(3)` times too wide. Measured against the
/// intended standard deviation (the code's own `sigma = radius / 2`):
///
/// | radius | intended | produced | over-blur |
/// |---|---|---|---|
/// | 2 px | 1.0 | 1.83 | 1.83x |
/// | 6 px | 3.0 | 4.83 | 1.61x |
/// | 12 px | 6.0 | 9.83 | 1.64x |
/// | 24 px | 12.0 | 20.83 | 1.74x |
///
/// Every shadow a theme drew was therefore roughly **1.6–1.8x softer than its own elevation said**,
/// which also made `Theme::elevation`'s levels less distinguishable from one another than the ladder
/// intends. With the share applied, the realised variance lands within the integer-width quantisation
/// of the target at every radius (`sigma=3`: 8.00 against 9.00, `sigma=6`: 34.00 against 36.00,
/// `sigma=12`: 140.00 against 144.00).
///
/// Returns `(0, 0, 0)` for a non-positive `sigma`, which the caller reads as "no blur": a zero
/// width is a no-op pass rather than a division by zero.
fn box_blur_widths(sigma: f32) -> (usize, usize, usize) {
    if sigma <= 0.0 {
        return (0, 0, 0);
    }
    // Each of the three passes carries an equal share of the total variance.
    const PASSES: f32 = 3.0;
    let per_pass = sigma / PASSES.sqrt();
    // The width that contributes exactly that share: `variance = (w^2 - 1) / 12`, so
    // `w = sqrt(12 * per_pass^2 + 1)`. Rounded down, then forced odd so the box has a true centre.
    let ideal = (12.0 * per_pass * per_pass + 1.0).sqrt();
    let wl = match ideal.floor() as usize {
        w if w % 2 == 1 => w,
        w => w.saturating_sub(1),
    };
    (wl, wl, wl + 2)
}

/// One box-blur pass over a single row (or column) of interleaved RGBA pixels.
///
/// The window sums are maintained incrementally: each step subtracts the pixel leaving the window
/// and adds the one entering it, so the pass costs one add and one subtract per channel per pixel
/// rather than `2r + 1` of each. Indices are clamped at both ends, which is what makes the edge
/// pixels blur against the image rather than against a shorter (and therefore darker) window.
///
/// # Why the sum reads from ``source`` and not from ``pixels``
///
/// A blur output pixel must be a function of the **input** pixels around it, and this pass writes its
/// output into the same row it reads. Sliding the window over `pixels` would therefore subtract a
/// pixel it had already overwritten, and the error compounds along the row — the *first* output
/// pixel is correct (nothing has been written yet) and every one after it drifts, which is the exact
/// shape of the failure the agreement test caught. `source` is the untouched row, held by the caller;
/// `pixels` is only ever written to.
///
/// `pixels` is the single row/column, `len` its pixel count, `width` the box width in pixels
/// (odd, and at least 2 by the caller's check), `source` the pre-pass copy of `pixels`.
fn blur_row(pixels: &mut [u8], len: usize, width: usize, source: &[u8]) {
    if len == 0 || width < 2 {
        return;
    }
    let half = width / 2;

    // The window for output pixel `x` spans the **clamped** index range `lo..=hi`.
    //
    // Deriving the divisor from the same two bounds the sum comes from is what makes an edge pixel
    // exact rather than approximately right; and `lo`/`hi` are also the two pixels the window
    // exchanges when it slides, so the sum and the slide cannot disagree about where the window is.
    let lo_of = |x: usize| x.saturating_sub(half);
    let hi_of = |x: usize| (x + half).min(len - 1);

    // Signed accumulators.
    //
    // The slide adds one pixel and subtracts another *before* dividing, and the subtracted one can be
    // the brighter of the two — so a `u32` sum underflows on the pixel where the window stops growing
    // and starts sliding, which is every real image. `i64` makes the intermediate honest and the
    // final value is provably in `0..=255` because it is a mean of bytes.
    let mut sums = [0i64; 4];
    // The window for pixel 0, which is `[0, half]` with the upper end clamped.
    for i in 0..=hi_of(0) {
        let idx = i * 4;
        for c in 0..4 {
            sums[c] += source[idx + c] as i64;
        }
    }

    for x in 0..len {
        let count = (hi_of(x) - lo_of(x) + 1) as i64;
        let out = x * 4;
        for c in 0..4 {
            pixels[out + c] = (sums[c] / count) as u8;
        }
        if x + 1 == len {
            break;
        }
        // Slide to the window for `x + 1`.
        //
        // # Why neither side of the slide is unconditional
        //
        // A window resting against an edge does not slide, it **resizes**, and treating it as a slide
        // is wrong in both directions:
        //
        // * Near the **left** edge the window grows: at `x = 0` it is `[0, half]` and at `x = 1` it is
        //   `[0, half + 1]`, so a pixel enters and none leaves. Subtracting `lo_of(x)` there removes a
        //   pixel that is still inside the window.
        // * Near the **right** edge it shrinks: the last pixel enters when `x + 1 + half` reaches
        //   `len - 1`, and after that each step only drops one from the left. Adding `hi_of(x + 1)`
        //   there adds the same pixel a second time.
        //
        // Both errors are *constant* once made rather than cumulative, so they read as a uniform shift
        // and a slightly bright tail rather than as an obvious tear — the reason this survived a naive
        // box-blur-to-box-blur comparison and only fell out against a direct convolution.
        //
        // A pixel enters while the window is still growing, and a pixel leaves once the left edge can
        // advance: `x >= half` is exactly when the left edge stops being pinned at 0, and
        // `x + 1 + half <= len - 1` is exactly when the right edge has not yet been pinned.
        let mut delta = [0i64; 4];
        if x + 1 + half < len {
            let entering = (x + 1 + half) * 4;
            for c in 0..4 {
                delta[c] += source[entering + c] as i64;
            }
        }
        if x >= half {
            let leaving = lo_of(x) * 4;
            for c in 0..4 {
                delta[c] -= source[leaving + c] as i64;
            }
        }
        for c in 0..4 {
            sums[c] += delta[c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `vec!` and `Vec` come from `compat` rather than the std prelude. These tests build pixel
    // buffers, and on the `ohos`/`android` cross targets there is no `std` prelude to supply them --
    // which is how they broke `check_harmony_cross.sh` (`cannot find type Vec in this scope`) while
    // every host build stayed green. `compat` re-exports `alloc::vec`, so importing it makes the
    // tests mean the same thing on every target rather than only on the ones with a `std`.
    use crate::compat::vec;
    use crate::compat::Vec;

    /// The blur's cost must not grow with the radius — that is the whole point of the sliding
    /// window, and it is a *timing* claim, so the measurement has to be built to survive a busy
    /// machine.
    ///
    /// Two things make it fragile if written naively, and both have been observed: a single sample
    /// of a fast blur is short enough that scheduler jitter is a large fraction of it, and
    /// measuring *all* of one radius before starting the other lets a burst of load inflate only
    /// the second. So the two radii are **interleaved** (load that arrives hits a sample of each)
    /// and each result is the **minimum** of several samples (the sample least disturbed by load).
    ///
    /// The threshold is a *shape* boundary, not a benchmark. Measured on this 400×400 surface in the
    /// debug profile this suite runs:
    ///
    ///   * sliding window (correct):      40 px costs ~2.9x a 2 px
    ///   * recomputed window (the bug):   40 px costs ~16.4x a 2 px
    ///
    /// `6.0` sits ~2x above the correct shape and ~2.7x below the bug, so the verdict does not
    /// depend on how fast the machine is. 4.0 was too close to 2.9: a single unlucky sample was
    /// enough to read a working blur as broken.
    #[test]
    fn the_blur_cost_does_not_grow_with_the_radius() {
        use core::time::Duration;
        use std::time::Instant;

        let (w, h) = (400usize, 400usize);
        // One buffer, reused. The blur's cost depends on the region and the radius, never on the
        // pixel values, so re-blurring the same bytes measures the same work — and it keeps the
        // fill (which `sin`s 640k values) out of the neighbour of the timed region, where it used
        // to be the larger term.
        let mut buf: Vec<u8> =
            (0..w * h * 4).map(|i| ((i as f32 * 0.37).sin().abs() * 255.0) as u8).collect();

        // One scratch set, reused exactly as the surface does at runtime (G-07).
        let mut scratch = crate::render::backend::surface::BoxBlurScratch::default();
        let mut blur = |radius: usize| {
            let start = Instant::now();
            box_blur_region(&mut buf, w, h, (0, 0, w, h), radius, &mut scratch);
            start.elapsed()
        };

        const SAMPLES: usize = 5;
        let mut small = Duration::MAX;
        let mut large = Duration::MAX;
        for _ in 0..SAMPLES {
            small = small.min(blur(2));
            large = large.min(blur(40));
        }

        let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 6.0,
            "a 40 px blur took {ratio:.1}x a 2 px one ({large:?} vs {small:?}, minimum of \
             {SAMPLES} samples) — the cost is still growing with the radius, so the window is not \
             sliding"
        );
    }

    // ── The Gaussian-approximating blur ─────────────────────────────────

    /// The sliding window must agree with the naive sum it replaced.
    ///
    /// This is the whole risk of the optimisation: an incremental window that adds and subtracts the
    /// wrong pixels is *plausible* — it produces a smooth-looking image — and wrong. So it is checked
    /// against a direct convolution, which is the definition, on a signal with no symmetry that could
    /// hide a mirrored index error.
    #[test]
    fn the_sliding_window_agrees_with_a_direct_convolution() {
        // A row of distinct values, so an off-by-one in either direction changes the output.
        let len = 23usize;
        let width = 5usize;
        let half = width / 2;
        let source: Vec<u8> =
            (0..len * 4).map(|i| ((i as f32 * 37.0).sin().abs() * 200.0 + 20.0) as u8).collect();
        let mut blurred = source.clone();
        blur_row(&mut blurred, len, width, &source);

        for x in 0..len {
            for c in 0..4 {
                // The reference: clamp the window at both ends *and* divide by the number of pixels
                // actually accumulated, which is what `blur_row` documents it does.
                let lo = x.saturating_sub(half);
                let hi = (x + half).min(len - 1);
                let sum: u32 = (lo..=hi).map(|k| source[k * 4 + c] as u32).sum();
                let expected = (sum / (hi - lo + 1) as u32) as u8;
                assert_eq!(
                    blurred[x * 4 + c],
                    expected,
                    "pixel {x} channel {c}: the window and the direct sum must agree"
                );
            }
        }
    }

    /// The window count must be exact, not merely close.
    ///
    /// A divisor that is off by one at an edge produces a subtle gradient rather than an obvious
    /// band, and the agreement test above would catch it only because it re-derives the same count.
    /// Pinning the count directly is what makes that test meaningful rather than circular.
    #[test]
    fn a_constant_field_survives_the_edge_clamping() {
        // A flat field must stay flat: every clamped window sums `count` copies of the same value, so
        // a divisor larger than `count` would darken the border and one smaller would lighten it.
        let len = 16usize;
        let mut row = vec![200u8; len * 4];
        blur_row(&mut row, len, 7, &vec![200u8; len * 4]);
        assert!(
            row.iter().all(|&v| v == 200),
            "a constant field must be unchanged by a blur, including at the edges"
        );
    }

    /// The three box widths must number three, be odd, and grow with the requested sigma.
    ///
    /// Odd widths give each box a true centre pixel, which is what makes the iterated kernel
    /// symmetric; a monotonic response is what makes `radius` mean something to a caller.
    #[test]
    fn the_box_widths_are_odd_and_monotonic() {
        let mut previous = 0usize;
        for radius in [1usize, 2, 4, 8, 16, 32, 64] {
            let (a, b, c) = box_blur_widths(radius as f32 / 2.0);
            assert!(a % 2 == 1 && b % 2 == 1 && c % 2 == 1, "widths must be odd: {a} {b} {c}");
            assert_eq!(a, b, "the pair must match, or the kernel is asymmetric");
            let reach = a + c;
            assert!(reach >= previous, "radius {radius} must not blur less than a smaller one");
            previous = reach;
        }
        assert_eq!(box_blur_widths(0.0), (0, 0, 0), "a zero sigma is not a blur");
        assert_eq!(box_blur_widths(-1.0), (0, 0, 0), "and neither is a negative one");
    }

    /// The three boxes together must realise the requested `sigma`, not three times its variance.
    ///
    /// # The defect this pins
    ///
    /// `box_blur_widths` solved each width from `variance = (w^2 - 1) / 12 == sigma^2`, i.e. it put
    /// the **whole** requested variance into **each** of the three passes. Variances add, so the
    /// kernel that came out had three times the intended variance -- about `sqrt(3)` times too wide,
    /// which measured out as every shadow being 1.6-1.8x softer than its own elevation said:
    ///
    /// | radius | intended sigma | produced |
    /// |---|---|---|
    /// | 2 px | 1.0 | 1.83 |
    /// | 6 px | 3.0 | 4.83 |
    /// | 12 px | 6.0 | 9.83 |
    /// | 24 px | 12.0 | 20.83 |
    ///
    /// The widths are still *odd* and still *monotonic* with such a bug present, which is exactly
    /// why `the_box_widths_are_odd_and_monotonic` above cannot see it: it asserts the shape of the
    /// response, not its scale. This test asserts the scale.
    ///
    /// # Why the radii below are the ladder's own, and why the band is wide at the small end
    ///
    /// A box width is an integer, so the realised variance can only land where some `(w^2 - 1) / 12`
    /// falls. The residual is therefore quantisation, and it shrinks as a fraction of the target as
    /// sigma grows. Measured after the fix, over exactly the four radii
    /// [`crate::render::default_shadow`] uses (`blur` 3/6/12/24):
    ///
    /// | blur | sigma | realised / target | standard deviation |
    /// |---|---|---|---|
    /// | 3 | 1.5 | 1.48 | 1.22x |
    /// | 6 | 3.0 | 0.889 | 0.94x |
    /// | 12 | 6.0 | 0.944 | 0.97x |
    /// | 24 | 12.0 | 0.972 | 0.99x |
    ///
    /// So the band is `0.6..=1.6` on variance (0.77x-1.26x on standard deviation): it admits the
    /// quantisation at `blur: 3`, and it still rejects the `sqrt(3)`-scale error by a wide margin --
    /// that bug produces 2.59-3.67x, i.e. outside the band at every radius.
    ///
    /// # Why `radius: 1` is deliberately not in the list
    ///
    /// Because at that radius the test **cannot tell the fix from the bug**, and a case that passes
    /// either way is not evidence. `box_blur_widths(0.5)` is `(1, 1, 3)` both before and after the
    /// fix -- every width floors to the same odd integers -- so both kernels realise 0.67 against a
    /// target of 0.25, a ratio of 2.667. Asserting it would either fail the *correct* code or force a
    /// band wide enough to admit the *broken* one, and both of those make the test worse than not
    /// having it.
    ///
    /// The smallest radius where a separating integer kernel exists is 2 (`sigma` 1.0), and the
    /// separation is complete from there up: fixed gives 0.667/1.48/0.83/0.89/0.92/0.94/0.97/0.99,
    /// buggy gives 3.33/3.56/3.67/2.59/2.92/2.69/3.01/3.04. Excluding one radius for a stated
    /// reason is the honest form of this exclusion; silently widening the band to cover it would
    /// have hidden exactly the defect the test exists for.
    #[test]
    fn the_three_boxes_realise_the_requested_sigma_not_three_times_it() {
        let variance_of = |w: usize| {
            if w == 0 {
                0.0
            } else {
                (w * w - 1) as f32 / 12.0
            }
        };

        // `default_shadow`'s four levels, plus a finer and a coarser radius so a regression that
        // only shows at one scale cannot hide behind the ladder's particular values. `radius: 1` is
        // excluded on purpose -- see this test's rustdoc.
        for radius in [2usize, 3, 4, 6, 8, 12, 24, 48, 96] {
            let sigma = radius as f32 / 2.0;
            let (a, b, c) = box_blur_widths(sigma);
            let realised = variance_of(a) + variance_of(b) + variance_of(c);
            let target = sigma * sigma;
            let ratio = realised / target;
            assert!(
                (0.6..=1.6).contains(&ratio),
                "radius {radius}: the kernel realises variance {realised:.2} against a target of \
                 {target:.2} (ratio {ratio:.3}), i.e. a standard deviation {:.2}x the one asked for. \
                 Widths were ({a}, {b}, {c}). A ratio near 3 means the per-pass variance share was \
                 dropped again.",
                ratio.sqrt()
            );
        }
    }

    /// The blur is separable: three boxes in x and three in y must equal the same work done the
    /// other way round.
    ///
    /// This is the property that lets the implementation do a row pass and a column pass instead of
    /// a two-dimensional kernel, and it is the one an accidental transposition would break.
    #[test]
    fn the_blur_is_separable() {
        let (w, h) = (9usize, 7usize);
        let source: Vec<u8> =
            (0..w * h * 4).map(|i| ((i as f32 * 91.0).cos().abs() * 180.0 + 30.0) as u8).collect();

        let mut horizontal_first = source.clone();
        box_blur_region(
            &mut horizontal_first,
            w,
            h,
            (0, 0, w, h),
            6,
            &mut crate::render::backend::surface::BoxBlurScratch::default(),
        );

        // The same region blurred after a vertical translation of the *source* by zero pixels is
        // the only honest comparison: `box_blur_region` always runs x then y, so "y then x" is
        // expressed by blurring a transposed copy and transposing back.
        let mut transposed = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                transposed[(x * h + y) * 4..(x * h + y) * 4 + 4]
                    .copy_from_slice(&source[(y * w + x) * 4..(y * w + x) * 4 + 4]);
            }
        }
        box_blur_region(
            &mut transposed,
            h,
            w,
            (0, 0, h, w),
            6,
            &mut crate::render::backend::surface::BoxBlurScratch::default(),
        );

        // A box blur is symmetric, so transposing the result back is a reference for the original.
        // Tolerated by one level: the three boxes are applied in a different order, so integer
        // truncation can land on either side of the exact value.
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    let a = horizontal_first[(y * w + x) * 4 + c] as i32;
                    let b = transposed[(x * h + y) * 4 + c] as i32;
                    assert!(
                        (a - b).abs() <= 1,
                        "({x},{y}) channel {c}: {a} vs {b} — the passes are not separable"
                    );
                }
            }
        }
    }
}
