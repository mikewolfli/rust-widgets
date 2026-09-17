// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Technical-analysis indicators, as pure functions over price series.
//!
//! # Why these are functions and not controls
//!
//! Every indicator here answers one shape of question — "given this series, what is the
//! derived series?" — and the answer depends on nothing but the numbers. There is no
//! geometry, no state, no event and no platform call involved.
//!
//! Building them as controls would mean a dozen controls that each own a copy of a
//! moving average, each needing a `WidgetKind`, a capability, a factory row and a
//! constructor, and each untestable without a window. As functions the arithmetic is
//! testable on its own, and the *drawing* stays one shared concern of the chart controls
//! that consume them (principle #24, infrastructure before controls). It is also what
//! makes the numbers auditable: a wrong MACD is caught by an assertion about a known
//! series, not by looking at a picture.
//!
//! # Padding convention
//!
//! Every function returns a vector the **same length as its input**, with positions that
//! are not yet computable filled with [`f64::NAN`]. An indicator needs a warm-up window —
//! a 20-period average has no value for the first 19 points — and returning a shorter
//! vector would force every caller to re-derive the alignment. Keeping the length means
//! output index `i` always corresponds to input index `i`, which the overlay code relies
//! on.
//!
//! `NAN` rather than `0.0` because zero is a *valid* price level; a leading run of zeroes
//! would draw a false line from the origin. The drawing code skips non-finite values, so
//! a warm-up gap reads as a gap rather than as data.
//!
//! # Non-finite input
//!
//! A `NAN` propagates into the window containing it and is then dropped: that window is
//! undefined and so is the output position. This is deliberate — a caller with a gap in
//! its data (a halted session, a missing tick) gets a gap in the indicator rather than a
//! silently interpolated value that looks like real analysis. `INFINITY` is treated the
//! same way, because an infinite price is a data error, not a level.

use alloc::vec;
use alloc::vec::Vec;

/// Simple moving average over `period` samples, padded with `NAN` through warm-up.
///
/// Returns all `NAN` when `period` is zero or longer than the input, because no position
/// has a full window — the honest answer, rather than a partial-window average that would
/// disagree with every other implementation.
///
/// # Why a rolling sum rather than a re-sum
///
/// Re-summing each window is `O(n * period)`; a trading year at period 200 is 50 000
/// samples x 200 adds, which is measurable inside a repaint. The running sum adds and
/// subtracts one term per sample instead. Subtracting is exact at these magnitudes — the
/// values are prices, not accumulated products — so the cheaper form is also the accurate
/// one here.
pub fn sma(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if period == 0 || period > values.len() {
        return out;
    }
    let mut sum = 0.0;
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            // A non-finite sample poisons every window containing it.
            sum = f64::NAN;
            continue;
        }
        if sum.is_nan() {
            // Re-derive once the bad sample has left the window.
            let window_start = index.saturating_sub(period - 1);
            let window = &values[window_start..=index];
            if !window.iter().all(|v| v.is_finite()) {
                continue;
            }
            sum = window.iter().sum();
        } else {
            sum += value;
            if index >= period {
                sum -= values[index - period];
            }
        }
        if index + 1 >= period {
            out[index] = sum / period as f64;
        }
    }
    out
}

/// Exponential moving average, seeded with the first valid sample.
///
/// Unlike [`sma`] this has a value from its first sample — the standard seeding is that
/// sample itself, so there is no warm-up gap. The consequence is that early values are
/// biased towards the seed; a warm-up of three to five times `period` is the usual
/// convention for trusting them, and that is the caller's decision rather than something
/// silently truncated here.
///
/// The smoothing factor is the conventional `2 / (period + 1)`. `period == 0` returns the
/// input unchanged, which is the identity the formula would otherwise divide by zero on.
pub fn ema(values: &[f64], period: usize) -> Vec<f64> {
    if period == 0 {
        return values.to_vec();
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut out = vec![f64::NAN; values.len()];
    let mut previous = f64::NAN;
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            // Break the chain: the next valid sample re-seeds rather than smearing the
            // previous value across the hole.
            previous = f64::NAN;
            continue;
        }
        previous =
            if previous.is_nan() { *value } else { alpha * value + (1.0 - alpha) * previous };
        out[index] = previous;
    }
    out
}

/// Wilder's smoothing, used by [`rsi`] and [`atr`].
///
/// This is an EMA with `alpha = 1 / period` rather than `2 / (period + 1)` — the original
/// Wilder form. It is a separate function rather than an `ema` argument because the two
/// are not interchangeable: substituting one for the other gives RSI values that differ
/// from every published table, which is exactly the silent disagreement this module
/// exists to prevent.
pub fn wilder_smooth(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if period == 0 {
        return out;
    }
    let mut previous = f64::NAN;
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            previous = f64::NAN;
            continue;
        }
        previous =
            if previous.is_nan() { *value } else { previous + (*value - previous) / period as f64 };
        out[index] = previous;
    }
    out
}

/// Relative Strength Index over `period` samples, in `0..=100`.
///
/// # Why the first `period` positions are `NAN`
///
/// RSI needs a change for every sample and a full window of changes before its first
/// average, so with `period = 14` the first value lands at index 14. Emitting anything
/// earlier would mean inventing the seed.
///
/// # Which RSI, stated
///
/// This implements Wilder's 1978 form: a plain mean of the first `period` gains and
/// losses, then Wilder smoothing thereafter. Many spreadsheet templates instead use a
/// simple moving average throughout, and the two disagree by a few tenths on the same
/// data. The choice is documented rather than hidden because "which RSI" is a real
/// question — anyone comparing against another platform will see the difference.
pub fn rsi(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if period == 0 || values.len() <= period {
        return out;
    }

    let mut gains = vec![0.0; values.len()];
    let mut losses = vec![0.0; values.len()];
    for index in 1..values.len() {
        let previous = values[index - 1];
        let current = values[index];
        if !previous.is_finite() || !current.is_finite() {
            gains[index] = f64::NAN;
            losses[index] = f64::NAN;
            continue;
        }
        let change = current - previous;
        gains[index] = change.max(0.0);
        losses[index] = (-change).max(0.0);
    }

    let seed_gains = &gains[1..=period];
    let seed_losses = &losses[1..=period];
    if !seed_gains.iter().all(|v| v.is_finite()) || !seed_losses.iter().all(|v| v.is_finite()) {
        return out;
    }
    let mut average_gain: f64 = seed_gains.iter().sum::<f64>() / period as f64;
    let mut average_loss: f64 = seed_losses.iter().sum::<f64>() / period as f64;
    out[period] = rsi_from(average_gain, average_loss);

    for index in (period + 1)..values.len() {
        if !gains[index].is_finite() || !losses[index].is_finite() {
            return out;
        }
        average_gain = (average_gain * (period as f64 - 1.0) + gains[index]) / period as f64;
        average_loss = (average_loss * (period as f64 - 1.0) + losses[index]) / period as f64;
        out[index] = rsi_from(average_gain, average_loss);
    }
    out
}

/// Converts an average gain/loss pair into an RSI reading.
///
/// A zero average loss means "only gains", which the ratio would divide by zero on; the
/// convention is to report `100`. A zero average gain with a real loss gives `0` through
/// the ordinary arithmetic, and both zero (a flat series) gives `50` — the neutral
/// reading rather than an undefined one.
fn rsi_from(average_gain: f64, average_loss: f64) -> f64 {
    if average_loss == 0.0 {
        return if average_gain == 0.0 { 50.0 } else { 100.0 };
    }
    let relative_strength = average_gain / average_loss;
    100.0 - (100.0 / (1.0 + relative_strength))
}

/// Moving Average Convergence/Divergence: `(macd, signal, histogram)`.
///
/// The three series a MACD pane draws, all the input's length:
///
/// * `macd` — `ema(fast) - ema(slow)`, the two averages the name refers to;
/// * `signal` — an EMA of the MACD line over `signal_period`, the slower line it is read
///   against;
/// * `histogram` — `macd - signal`, the bar series; drawn as the familiar red/green
///   columns and, being the difference of two lines, the one that crosses zero.
///
/// Returning all three rather than only the MACD line is not a convenience: a pane showing
/// the line without its signal line cannot be interpreted, so splitting them across three
/// calls would make the common case three calls that can drift out of alignment.
pub fn macd(
    values: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let fast = ema(values, fast_period);
    let slow = ema(values, slow_period);
    let line: Vec<f64> = fast
        .iter()
        .zip(slow.iter())
        .map(|(f, s)| if f.is_finite() && s.is_finite() { f - s } else { f64::NAN })
        .collect();
    let signal = ema(&line, signal_period);
    let histogram: Vec<f64> = line
        .iter()
        .zip(signal.iter())
        .map(|(m, s)| if m.is_finite() && s.is_finite() { m - s } else { f64::NAN })
        .collect();
    (line, signal, histogram)
}

/// Bollinger Bands: `(lower, middle, upper)`.
///
/// The middle band is the `period` simple moving average and the outer bands sit
/// `multiplier` standard deviations from it. All three are the input's length, `NAN`
/// through the warm-up.
///
/// # Population, not sample, deviation
///
/// The two differ by `sqrt(n / (n - 1))` — about 3% at period 20 — and Bollinger's own
/// formulation uses the population form. The sample form is a common slip that makes the
/// bands a few percent too wide and no longer matches any published chart;
/// `band_width_is_the_population_deviation` pins the choice at a period where the two
/// are 15% apart, so it cannot pass by rounding.
pub fn bollinger_bands(
    values: &[f64],
    period: usize,
    multiplier: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let middle = sma(values, period);
    let mut lower = vec![f64::NAN; values.len()];
    let mut upper = vec![f64::NAN; values.len()];

    if period == 0 {
        return (lower, middle, upper);
    }
    for index in 0..values.len() {
        if !middle[index].is_finite() {
            continue;
        }
        let window = &values[index + 1 - period..=index];
        if !window.iter().all(|v| v.is_finite()) {
            continue;
        }
        let mean = middle[index];
        let variance = window.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / period as f64;
        let deviation = variance.sqrt();
        lower[index] = mean - multiplier * deviation;
        upper[index] = mean + multiplier * deviation;
    }
    (lower, middle, upper)
}

/// Stochastic oscillator: `(%K, %D)`.
///
/// `%K` is where the close sits within the recent high/low range as a percentage; `%D` is
/// its `d_period` moving average, the signal line that is actually read. Both are the
/// input's length and `NAN` through the warm-up.
///
/// # The `k_smoothing` argument
///
/// Slow stochastic (`k_smoothing = 3`, the default in most platform software) is the fast
/// `%K` averaged again; fast stochastic is `k_smoothing = 1`. Both are in use, so the
/// choice is a parameter rather than a hard-coded constant.
///
/// A zero-width range — every period trading at one level, which happens on an illiquid
/// instrument — puts the close at the midpoint by convention rather than dividing by zero.
pub fn stochastic(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    period: usize,
    k_smoothing: usize,
    d_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let length = closes.len().min(highs.len()).min(lows.len());
    let mut raw_k = vec![f64::NAN; length];

    if period == 0 {
        return (vec![f64::NAN; length], vec![f64::NAN; length]);
    }
    // Indexed by the close's own position, because the window extends backwards from it
    // and the result is written back at that index; `iter_mut().enumerate()` would need a
    // skip that obscures the alignment the whole indicator depends on.
    for (window_start, slot) in raw_k.iter_mut().enumerate().skip(period - 1) {
        let window_high = &highs[window_start + 1 - period..=window_start];
        let window_low = &lows[window_start + 1 - period..=window_start];
        let close = closes[window_start];
        if !window_high.iter().all(|v| v.is_finite())
            || !window_low.iter().all(|v| v.is_finite())
            || !close.is_finite()
        {
            continue;
        }
        let highest = window_high.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lowest = window_low.iter().copied().fold(f64::INFINITY, f64::min);
        let range = highest - lowest;
        *slot = if range == 0.0 { 50.0 } else { (close - lowest) / range * 100.0 };
    }

    let k = if k_smoothing <= 1 { raw_k } else { sma(&raw_k, k_smoothing) };
    let d = sma(&k, d_period);
    (k, d)
}

/// The per-sample true range series, with index 0 undefined.
///
/// Split out because [`atr`] and any future range-based indicator need the same
/// definition, and two copies of it would eventually disagree about the first sample or
/// about how gaps are counted.
fn true_range(highs: &[f64], lows: &[f64], closes: &[f64], length: usize) -> Vec<f64> {
    let mut ranges = vec![f64::NAN; length];
    for index in 1..length {
        let high = highs[index];
        let low = lows[index];
        let previous_close = closes[index - 1];
        if !high.is_finite() || !low.is_finite() || !previous_close.is_finite() {
            continue;
        }
        ranges[index] =
            (high - low).max((high - previous_close).abs()).max((low - previous_close).abs());
    }
    ranges
}

/// Average True Range over `period` samples.
///
/// The true range is the largest of the high/low span, the gap from the previous close to
/// this high, and the gap from that close to this low. The maximum is what makes it count
/// gaps — a limit-up open is a real move a plain high/low span would miss entirely.
///
/// Seeded with the mean of the first `period` true ranges, then Wilder-smoothed, so the
/// first value lands at index `period`.
pub fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let length = closes.len().min(highs.len()).min(lows.len());
    let mut out = vec![f64::NAN; length];
    if period == 0 || length <= period {
        return out;
    }

    let ranges = true_range(highs, lows, closes, length);
    let seed_ranges = &ranges[1..=period];
    if !seed_ranges.iter().all(|v| v.is_finite()) {
        return out;
    }
    let mut average: f64 = seed_ranges.iter().sum::<f64>() / period as f64;
    out[period] = average;

    for index in (period + 1)..length {
        if !ranges[index].is_finite() {
            return out;
        }
        average = (average * (period as f64 - 1.0) + ranges[index]) / period as f64;
        out[index] = average;
    }
    out
}

/// The highest high and lowest low over a trailing `period`, as `(highs, lows)`.
///
/// The building block for Donchian channels, breakout markers and a K-line pane's own
/// price axis. Both series are the input's length and `NAN` until a full window exists,
/// and both are computed together because a caller that wants one almost always wants the
/// other — a channel is two lines.
///
/// # Why a monotonic deque
///
/// A window scan is `O(n * period)`, which a chart repaint cannot afford at period 200.
/// The deque holds indices with decreasing values, so its front is always the window
/// maximum, and each index enters and leaves exactly once — `O(n)` overall.
/// `trailing_extremes_match_a_naive_scan` asserts the two agree, so the optimisation
/// cannot quietly change an answer.
pub fn trailing_extremes(highs: &[f64], lows: &[f64], period: usize) -> (Vec<f64>, Vec<f64>) {
    let length = highs.len().min(lows.len());
    let mut out_high = vec![f64::NAN; length];
    let mut out_low = vec![f64::NAN; length];
    if period == 0 || period > length {
        return (out_high, out_low);
    }

    // Indices with values strictly decreasing front-to-back: front is the maximum.
    let mut max_deque: Vec<usize> = Vec::with_capacity(period);
    // Indices with values strictly increasing front-to-back: front is the minimum.
    let mut min_deque: Vec<usize> = Vec::with_capacity(period);

    for index in 0..length {
        while max_deque.first().is_some_and(|front| *front + period <= index) {
            max_deque.remove(0);
        }
        while min_deque.first().is_some_and(|front| *front + period <= index) {
            min_deque.remove(0);
        }
        if highs[index].is_finite() {
            while max_deque.last().is_some_and(|back| highs[*back] <= highs[index]) {
                max_deque.pop();
            }
            max_deque.push(index);
        }
        if lows[index].is_finite() {
            while min_deque.last().is_some_and(|back| lows[*back] >= lows[index]) {
                min_deque.pop();
            }
            min_deque.push(index);
        }

        if index + 1 < period {
            continue;
        }
        let window_start = index + 1 - period;
        let clean = highs[window_start..=index].iter().all(|v| v.is_finite())
            && lows[window_start..=index].iter().all(|v| v.is_finite());
        if !clean {
            continue;
        }
        // Every finite value was pushed and only window indices survive, so a clean
        // window guarantees a front exists.
        if let Some(front) = max_deque.first() {
            out_high[index] = highs[*front];
        }
        if let Some(front) = min_deque.first() {
            out_low[index] = lows[*front];
        }
    }
    (out_high, out_low)
}

/// Donchian channel: `(lower, middle, upper)`.
///
/// The `period` highest high, the `period` lowest low, and their midpoint. A breakout
/// above the upper band is the classic trend-following signal, which is why the middle
/// line — the channel's own centre — is included: a close above it but below the upper
/// band is a weaker reading, and that distinction cannot be made from the bands alone.
pub fn donchian_channel(
    highs: &[f64],
    lows: &[f64],
    period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (upper, lower) = trailing_extremes(highs, lows, period);
    let middle: Vec<f64> = upper
        .iter()
        .zip(lower.iter())
        .map(|(u, l)| if u.is_finite() && l.is_finite() { (u + l) / 2.0 } else { f64::NAN })
        .collect();
    (lower, middle, upper)
}

/// The volume-weighted average price over a trailing `period`.
///
/// `sum(typical_price * volume) / sum(volume)`, where the typical price is
/// `(high + low + close) / 3`. The typical price rather than the close is the convention
/// because an intraday VWAP should reflect where trade actually happened, not only where
/// the bar ended.
///
/// A window whose total volume is zero is undefined — the division has no meaning — and
/// reports `NAN` rather than carrying the previous value forward, which would be a
/// made-up level.
pub fn vwap(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[f64],
    period: usize,
) -> Vec<f64> {
    let length = closes.len().min(highs.len()).min(lows.len()).min(volumes.len());
    let mut out = vec![f64::NAN; length];
    if period == 0 || period > length {
        return out;
    }
    // Iterated through `out` so the value is written at the same position the window
    // ends at; the index is the window's last bar, not an offset into it.
    for (index, slot) in out.iter_mut().enumerate().skip(period - 1) {
        let window_start = index + 1 - period;
        let mut weighted = 0.0;
        let mut total_volume = 0.0;
        let mut clean = true;
        for offset in window_start..=index {
            let high = highs[offset];
            let low = lows[offset];
            let close = closes[offset];
            let volume = volumes[offset];
            if !high.is_finite() || !low.is_finite() || !close.is_finite() || !volume.is_finite() {
                clean = false;
                break;
            }
            let typical = (high + low + close) / 3.0;
            weighted += typical * volume;
            total_volume += volume;
        }
        if clean && total_volume != 0.0 {
            *slot = weighted / total_volume;
        }
    }
    out
}

/// Money Flow Index over `period` samples, in `0..=100`.
///
/// The volume-weighted sibling of RSI: where RSI weighs up and down moves equally, MFI
/// scales them by the volume that accompanied them. Read as overbought above 80 and
/// oversold below 20, the same bands as RSI.
///
/// A positive money flow is the typical price times volume on bars whose typical price
/// rose; the negative flow is the same on bars whose typical price fell, and is
/// accumulated as a positive magnitude so the ratio is the usual one. An unchanged
/// typical price adds to neither side, which is the definition.
pub fn money_flow_index(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[f64],
    period: usize,
) -> Vec<f64> {
    let length = closes.len().min(highs.len()).min(lows.len()).min(volumes.len());
    let mut out = vec![f64::NAN; length];
    if period == 0 || length <= period {
        return out;
    }

    let mut positive = vec![0.0; length];
    let mut negative = vec![0.0; length];
    for index in 1..length {
        let high = highs[index];
        let low = lows[index];
        let close = closes[index];
        let volume = volumes[index];
        if !high.is_finite() || !low.is_finite() || !close.is_finite() || !volume.is_finite() {
            positive[index] = f64::NAN;
            negative[index] = f64::NAN;
            continue;
        }
        let typical = (high + low + close) / 3.0;
        let previous_typical = (highs[index - 1] + lows[index - 1] + closes[index - 1]) / 3.0;
        if !previous_typical.is_finite() {
            positive[index] = f64::NAN;
            negative[index] = f64::NAN;
            continue;
        }
        let flow = typical * volume;
        if typical > previous_typical {
            positive[index] = flow;
        } else if typical < previous_typical {
            negative[index] = flow;
        }
    }

    for index in period..length {
        let window_start = index + 1 - period;
        let positive_slice = &positive[window_start..=index];
        let negative_slice = &negative[window_start..=index];
        if !positive_slice.iter().all(|v| v.is_finite())
            || !negative_slice.iter().all(|v| v.is_finite())
        {
            continue;
        }
        let total_positive: f64 = positive_slice.iter().sum();
        let total_negative: f64 = negative_slice.iter().sum();
        out[index] = if total_negative == 0.0 {
            if total_positive == 0.0 {
                50.0
            } else {
                100.0
            }
        } else {
            let ratio = total_positive / total_negative;
            100.0 - (100.0 / (1.0 + ratio))
        };
    }
    out
}

/// On-Balance Volume: a running total that adds volume on up bars and subtracts it on
/// down bars.
///
/// Not padded — OBV is defined from its first sample, seeded with that bar's volume —
/// because a cumulative series has no warm-up window.
///
/// # What a missing bar does
///
/// A non-finite sample produces `NAN` at its own index, and the total carries forward
/// from there. It does **not** silently count as "unchanged": whether that bar rose or
/// fell is exactly what the missing data would have told us, so claiming it contributed
/// nothing is a guess dressed up as data.
///
/// The next bar is undefined too, because its direction is measured against a close that
/// does not exist — the same reasoning as RSI stopping at a gap. A caller with holes in
/// its data gets holes in the indicator, which is honest, rather than a flat run that
/// looks like a quiet market. The total carries on from the last known value once a
/// sample **and** its predecessor are both real.
pub fn on_balance_volume(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let length = closes.len().min(volumes.len());
    let mut out = vec![f64::NAN; length];
    if length == 0 {
        return out;
    }
    let mut total = 0.0;
    for index in 0..length {
        if !closes[index].is_finite() || !volumes[index].is_finite() {
            // The bar itself is unknown; the running total has not moved.
            continue;
        }
        if index == 0 {
            total = volumes[index];
        } else if !closes[index - 1].is_finite() {
            // The direction needs the previous close, and there is not one. Report the
            // gap rather than treating a comparison against `NAN` — which is false in
            // both directions — as "unchanged".
            continue;
        } else if closes[index] > closes[index - 1] {
            total += volumes[index];
        } else if closes[index] < closes[index - 1] {
            total -= volumes[index];
        }
        out[index] = total;
    }
    out
}

/// The lowest and highest finite value in a series, for axis scaling.
///
/// A convenience every financial pane needs: a K-line pane and its volume pane must share
/// a horizontal index range, and computing the vertical extent in one place keeps the two
/// from disagreeing. Non-finite samples are ignored; an all-gap input returns
/// `(NAN, NAN)`, which the drawing code reads as "nothing to draw".
pub fn series_extent(values: &[f64]) -> (f64, f64) {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for value in values.iter().filter(|v| v.is_finite()) {
        low = low.min(*value);
        high = high.max(*value);
    }
    if low > high {
        (f64::NAN, f64::NAN)
    } else {
        (low, high)
    }
}

/// Whether any value in a series is a usable number.
///
/// Lets a drawing routine answer "is there anything to draw" in one call instead of
/// scanning for finiteness at each of the several places that need it.
pub fn has_drawable_values(values: &[f64]) -> bool {
    values.iter().any(|value| value.is_finite())
}

/// Pads a shorter series up to `length` on the **left** with `NAN`.
///
/// The alignment helper for a pane drawing a derived series beside a price series: an
/// indicator whose first value is at index 14 of a 100-bar window must be drawn starting
/// at bar 14, not stretched across all 100. Left-padding is the correct direction because
/// an indicator's gap is always at the *start* — it needs history that does not exist yet.
///
/// A series already at or beyond `length` is truncated, so a caller with more data than
/// the window shows gets the leading part that fits rather than a length mismatch.
pub fn align_left(values: &[f64], length: usize) -> Vec<f64> {
    if values.len() >= length {
        return values[..length].to_vec();
    }
    let mut out = vec![f64::NAN; length];
    let offset = length - values.len();
    out[offset..].copy_from_slice(values);
    out
}

/// Test module for the indicator arithmetic.
///
/// # Why every assertion is against a hand-computable series
///
/// A recorded-output golden file would only prove the code still does what it did,
/// including any mistake it made the first time. Each fixture here is small enough that
/// the expected value can be checked by hand from the definition, so an assertion that
/// passes means the arithmetic is right rather than merely unchanged.
///
/// # Why the edge cases dominate
///
/// The interesting failure modes of these functions are not the ordinary window — that
/// is a sum — but the boundaries: a warm-up that must be `NAN` rather than zero, a gap
/// that must not be interpolated, a flat series that must not divide by zero, and a
/// period larger than the data. Those are what most of these tests exercise.
#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts two values agree to well inside a price tick.
    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    /// A moving average is undefined until its window is full.
    #[test]
    fn a_moving_average_warms_up_over_its_window() {
        let result = sma(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert!(result[0].is_nan(), "one sample cannot fill a 3-period window");
        assert!(result[1].is_nan(), "two samples cannot either");
        assert!(close(result[2], 2.0), "mean of 1,2,3");
        assert!(close(result[3], 3.0), "mean of 2,3,4");
        assert!(close(result[4], 4.0), "mean of 3,4,5");
    }

    /// A period longer than the input has no full window anywhere.
    #[test]
    fn a_moving_average_longer_than_the_series_is_all_gaps() {
        let result = sma(&[1.0, 2.0], 5);
        assert_eq!(result.len(), 2, "the length must always match the input");
        assert!(result.iter().all(|value| value.is_nan()));
    }

    /// A zero period is undefined rather than a pass-through.
    #[test]
    fn a_zero_period_moving_average_produces_no_values() {
        let result = sma(&[1.0, 2.0, 3.0], 0);
        assert!(result.iter().all(|value| value.is_nan()));
    }

    /// The fast rolling sum must agree with a naive re-sum on a non-trivial series.
    #[test]
    fn the_rolling_sum_matches_a_naive_resum() {
        let values: Vec<f64> =
            (0..50).map(|index| (index as f64 * 0.37).sin() * 10.0 + 20.0).collect();
        let rolling = sma(&values, 7);
        for index in 6..values.len() {
            let naive: f64 = values[index - 6..=index].iter().sum::<f64>() / 7.0;
            assert!(close(rolling[index], naive), "index {index} must agree with a re-sum");
        }
    }

    /// A gap in the input must produce a gap in the output, not an interpolated value.
    #[test]
    fn a_non_finite_sample_produces_a_gap_rather_than_a_guess() {
        let result = sma(&[1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0], 3);
        assert!(result[4].is_nan(), "the window containing NaN is undefined");
        assert!(result[5].is_finite(), "and recovers once the bad sample leaves");
        assert!(close(result[5], 5.0), "mean of 4,5,6");
    }

    /// An infinate sample is a data error, not a price level.
    #[test]
    fn an_infinite_sample_is_treated_as_a_gap() {
        let result = sma(&[1.0, f64::INFINITY, 3.0], 2);
        assert!(result[1].is_nan(), "the window containing infinity is undefined");
        assert!(result[2].is_nan(), "and the next window still contains it");
    }

    /// An EMA is defined from its first sample and holds a flat series exactly.
    #[test]
    fn an_exponential_average_starts_at_the_first_sample() {
        let result = ema(&[10.0, 10.0, 10.0, 10.0], 3);
        assert!(close(result[0], 10.0), "seeded with the first value");
        assert!(result.iter().all(|value| close(*value, 10.0)), "a flat series stays flat");
    }

    /// The EMA moves towards a step by the conventional alpha.
    #[test]
    fn an_exponential_average_moves_by_its_smoothing_factor() {
        // period 3 gives alpha = 2 / 4 = 0.5, so a step from 0 to 10 lands at 5.
        let result = ema(&[0.0, 10.0], 3);
        assert!(close(result[1], 5.0), "half the step, got {}", result[1]);
    }

    /// A zero-period EMA is the identity rather than a division by zero.
    #[test]
    fn a_zero_period_exponential_average_returns_the_input() {
        let values = [3.0, 1.0, 4.0];
        assert_eq!(ema(&values, 0), values.to_vec());
    }

    /// A gap breaks the EMA chain rather than smearing a value across it.
    #[test]
    fn a_gap_re_seeds_an_exponential_average() {
        let result = ema(&[10.0, f64::NAN, 20.0, 20.0], 3);
        assert!(close(result[2], 20.0), "the sample after a gap becomes the seed");
        assert!(close(result[3], 20.0));
    }

    /// Wilder smoothing is defined immediately and approaches the input.
    #[test]
    fn wilder_smoothing_is_defined_immediately() {
        let result = wilder_smooth(&[5.0, 6.0, 7.0], 3);
        assert!(close(result[0], 5.0));
        assert!(result[1] > 5.0 && result[1] < 6.0, "it moves towards the sample");
        assert!(result[2] > result[1], "and keeps moving");
    }

    /// RSI on a strictly rising series saturates at 100 — there are no losses.
    #[test]
    fn rsi_of_a_rising_series_saturates_at_one_hundred() {
        let values: Vec<f64> = (0..30).map(|index| 100.0 + index as f64).collect();
        let result = rsi(&values, 14);
        assert!(result[..14].iter().all(|value| value.is_nan()), "warm-up is undefined");
        assert!(close(result[14], 100.0), "no losses means the maximum reading");
        assert!(close(result[29], 100.0));
    }

    /// RSI on a strictly falling series bottoms at 0 — there are no gains.
    #[test]
    fn rsi_of_a_falling_series_bottoms_at_zero() {
        let values: Vec<f64> = (0..30).map(|index| 100.0 - index as f64).collect();
        let result = rsi(&values, 14);
        assert!(close(result[14], 0.0), "no gains means the minimum reading");
    }

    /// A flat series has no gains and no losses, which reads neutral.
    #[test]
    fn rsi_of_a_flat_series_is_neutral() {
        let result = rsi(&[50.0; 30], 14);
        assert!(close(result[14], 50.0), "equal gains and losses is the midpoint");
        assert!(close(result[29], 50.0));
    }

    /// RSI stays inside its published bounds on mixed data.
    #[test]
    fn rsi_stays_within_zero_and_one_hundred() {
        let values: Vec<f64> = (0..200)
            .map(|index| 100.0 + (index as f64 * 0.7).sin() * 5.0 + (index as f64 * 0.13).cos())
            .collect();
        for value in rsi(&values, 14).iter().filter(|value| value.is_finite()) {
            assert!((0.0..=100.0).contains(value), "RSI out of range: {value}");
        }
    }

    /// RSI needs strictly more samples than its period.
    #[test]
    fn rsi_needs_more_samples_than_its_period() {
        assert!(rsi(&[1.0, 2.0, 3.0], 14).iter().all(|value| value.is_nan()));
        assert!(rsi(&[1.0, 2.0, 3.0], 0).iter().all(|value| value.is_nan()));
    }

    /// A gap in the data gives a gap in the reading rather than a carried-forward value.
    #[test]
    fn rsi_stops_at_a_gap_rather_than_carrying_a_value_forward() {
        let mut values: Vec<f64> =
            (0..60).map(|index| 100.0 + (index as f64 * 0.4).sin()).collect();
        assert!(rsi(&values, 14)[59].is_finite(), "the fixture is clean to begin with");
        values[50] = f64::NAN;
        let result = rsi(&values, 14);
        assert!(result[50].is_nan(), "the window containing the gap is undefined");
        assert!(result[59].is_nan(), "and the computation stops there rather than resuming");
    }

    /// MACD's histogram is the difference of its two lines, by definition.
    #[test]
    fn macd_histogram_is_the_difference_of_its_two_lines() {
        let values: Vec<f64> =
            (0..120).map(|index| 100.0 + (index as f64 * 0.3).sin() * 8.0).collect();
        let (line, signal, histogram) = macd(&values, 12, 26, 9);
        assert_eq!(line.len(), values.len());
        assert_eq!(signal.len(), values.len());
        assert_eq!(histogram.len(), values.len());
        for index in 0..values.len() {
            if line[index].is_finite() && signal[index].is_finite() {
                assert!(
                    close(histogram[index], line[index] - signal[index]),
                    "index {index}: histogram must be line minus signal"
                );
            }
        }
    }

    /// A faster EMA leads a slower one in an uptrend, which makes the MACD line positive.
    #[test]
    fn macd_is_positive_while_price_rises() {
        let values: Vec<f64> = (0..80).map(|index| 100.0 + index as f64 * 2.0).collect();
        let (line, _, _) = macd(&values, 12, 26, 9);
        assert!(
            line.last().is_some_and(|value| *value > 0.0),
            "a steady rise gives a positive MACD"
        );
    }

    /// A zero signal period leaves the MACD line as its own signal, so the histogram is
    /// zero rather than undefined.
    #[test]
    fn macd_with_no_signal_period_has_a_zero_histogram() {
        let values: Vec<f64> = (0..40).map(|index| 100.0 + index as f64).collect();
        let (line, signal, histogram) = macd(&values, 3, 6, 0);
        assert_eq!(line, signal, "a zero-period EMA is the identity");
        assert!(
            histogram.iter().all(|value| !value.is_finite() || close(*value, 0.0)),
            "so the histogram collapses to zero"
        );
    }

    /// The bands sit symmetrically around the middle band.
    #[test]
    fn bollinger_bands_are_symmetric_about_the_middle() {
        let values: Vec<f64> =
            (0..60).map(|index| 100.0 + (index as f64 * 0.5).sin() * 6.0).collect();
        let (lower, middle, upper) = bollinger_bands(&values, 20, 2.0);
        for index in 19..values.len() {
            if !middle[index].is_finite() {
                continue;
            }
            assert!(lower[index] < middle[index], "the lower band sits below the mean");
            assert!(upper[index] > middle[index], "the upper band sits above it");
            assert!(
                close(middle[index] - lower[index], upper[index] - middle[index]),
                "index {index}: the bands must be equidistant"
            );
        }
    }

    /// The band width uses the population deviation, not the sample deviation.
    ///
    /// At period 4 the two differ by 15%, which makes the distinction an assertion rather
    /// than a rounding question.
    #[test]
    fn band_width_is_the_population_deviation() {
        // Window [4,4,5,5]: mean 4.5, population deviation 0.5, sample deviation 0.577.
        let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let (lower, middle, upper) = bollinger_bands(&values, 4, 1.0);
        assert!(close(middle[5], 4.5), "the middle band is the mean");
        assert!(close(upper[5] - 4.5, 0.5), "population deviation is 0.5, not 0.577");
        assert!(close(4.5 - lower[5], 0.5));
    }

    /// Bands are gaps until the window is full.
    #[test]
    fn bollinger_bands_are_gaps_during_warm_up() {
        let values: Vec<f64> = (0..10).map(|index| index as f64).collect();
        let (lower, middle, upper) = bollinger_bands(&values, 5, 2.0);
        assert!(lower[3].is_nan() && middle[3].is_nan() && upper[3].is_nan());
        assert!(middle[4].is_finite(), "the first full window is at index 4");
    }

    /// A flat series has zero deviation, so all three bands collapse onto one line.
    #[test]
    fn bollinger_bands_of_a_flat_series_collapse() {
        let (lower, middle, upper) = bollinger_bands(&[7.0; 20], 5, 2.0);
        assert!(close(lower[10], 7.0) && close(middle[10], 7.0) && close(upper[10], 7.0));
    }

    /// %K reads 100 at the top of the range.
    #[test]
    fn stochastic_reads_the_top_of_the_range() {
        let highs = [10.0, 12.0, 14.0, 16.0, 18.0];
        let lows = [8.0, 10.0, 12.0, 14.0, 16.0];
        let closes = [18.0; 5];
        let (k, _) = stochastic(&highs, &lows, &closes, 5, 1, 3);
        assert!(close(k[4], 100.0), "a close at the high reads 100");
    }

    /// %K reads 0 at the bottom of the range.
    #[test]
    fn stochastic_reads_the_bottom_of_the_range() {
        let highs = [10.0, 12.0, 14.0, 16.0, 18.0];
        let lows = [8.0, 10.0, 12.0, 14.0, 16.0];
        let closes = [8.0; 5];
        let (k, _) = stochastic(&highs, &lows, &closes, 5, 1, 3);
        assert!(close(k[4], 0.0), "a close at the low reads 0");
    }

    /// A zero-width range reads neutral instead of dividing by zero.
    #[test]
    fn stochastic_of_a_zero_range_reads_neutral() {
        let (k, _) = stochastic(&[5.0; 5], &[5.0; 5], &[5.0; 5], 3, 1, 3);
        assert!(close(k[2], 50.0), "no range at all is the midpoint");
    }

    /// A zero period produces no readings rather than a wrong one.
    #[test]
    fn stochastic_with_a_zero_period_produces_nothing() {
        let (k, d) = stochastic(&[1.0; 4], &[1.0; 4], &[1.0; 4], 0, 1, 3);
        assert!(k.iter().all(|value| value.is_nan()));
        assert!(d.iter().all(|value| value.is_nan()));
    }

    /// The true range accounts for a gap that a plain high/low span would miss.
    #[test]
    fn true_range_accounts_for_gaps() {
        // Bar 1 opens far above bar 0's close, so the gap dominates the range.
        let ranges = true_range(&[10.0, 30.0], &[9.0, 28.0], &[9.5, 29.0], 2);
        assert!(close(ranges[1], 20.5), "30 - 9.5, not the 30 - 28 span");
    }

    /// ATR averages the true range and is defined from index `period`.
    #[test]
    fn atr_is_defined_after_its_period() {
        let highs: Vec<f64> = (0..20).map(|index| 10.0 + index as f64).collect();
        let lows: Vec<f64> = (0..20).map(|index| 9.0 + index as f64).collect();
        let closes: Vec<f64> = (0..20).map(|index| 9.5 + index as f64).collect();
        let result = atr(&highs, &lows, &closes, 5);
        assert!(result[..5].iter().all(|value| value.is_nan()), "warm-up");
        assert!(result[5].is_finite(), "the first value is at index `period`");
        // Each bar spans 1.0, but it also opens 1.5 above the previous close (the bars rise
        // by 1.0 and each closes at its own midpoint), so the true range is 1.5 — not the
        // 1.0 span. This is the whole reason `true_range` exists rather than using `high - low`.
        assert!(close(result[5], 1.5), "got {}", result[5]);
    }

    /// ATR sees the gap that a high/low span would hide.
    #[test]
    fn atr_is_larger_than_the_high_low_span_when_a_gap_exists() {
        let highs = [10.0, 30.0, 30.5];
        let lows = [9.0, 28.0, 28.5];
        let closes = [9.5, 29.0, 29.5];
        let result = atr(&highs, &lows, &closes, 2);
        // The true ranges are max(2, 20.5, 18.5) = 20.5 and max(2, 1.5, 0.5) = 2.0, so the
        // seeding mean is (20.5 + 2.0) / 2 = 11.25 — far above the 2.0 high/low span.
        assert!(close(result[2], 11.25), "got {}", result[2]);
    }

    /// A period longer than the series gives no ATR rather than a partial one.
    #[test]
    fn atr_needs_more_samples_than_its_period() {
        let result = atr(&[1.0, 2.0], &[1.0, 2.0], &[1.0, 2.0], 5);
        assert!(result.iter().all(|value| value.is_nan()));
    }

    /// Trailing extremes use a window, and the window moves.
    #[test]
    fn trailing_extremes_follow_the_window() {
        let highs = [1.0, 5.0, 2.0, 2.0, 2.0];
        let lows = [1.0, 4.0, 0.5, 0.5, 0.5];
        let (upper, lower) = trailing_extremes(&highs, &lows, 2);
        assert!(close(upper[1], 5.0), "window [1,1] includes the spike");
        assert!(close(upper[2], 5.0), "window [1,2] still includes it");
        assert!(close(upper[3], 2.0), "the spike has left the window");
        assert!(close(lower[2], 0.5), "the low is picked up as it arrives");
    }

    /// The deque implementation must agree with a naive window scan everywhere.
    #[test]
    fn trailing_extremes_match_a_naive_scan() {
        let highs: Vec<f64> =
            (0..200).map(|index| 50.0 + (index as f64 * 0.31).sin() * 10.0).collect();
        let lows: Vec<f64> = highs.iter().map(|high| high - 2.0).collect();
        let (upper, lower) = trailing_extremes(&highs, &lows, 13);
        for index in 12..highs.len() {
            let expected_high =
                highs[index - 12..=index].iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let expected_low =
                lows[index - 12..=index].iter().copied().fold(f64::INFINITY, f64::min);
            assert!(close(upper[index], expected_high), "index {index} high");
            assert!(close(lower[index], expected_low), "index {index} low");
        }
    }

    /// A period longer than the series gives no extremes.
    #[test]
    fn trailing_extremes_need_a_full_window() {
        let (upper, lower) = trailing_extremes(&[1.0, 2.0], &[1.0, 2.0], 5);
        assert!(upper.iter().all(|value| value.is_nan()));
        assert!(lower.iter().all(|value| value.is_nan()));
    }

    /// The Donchian middle line is the channel centre.
    #[test]
    fn donchian_middle_is_the_channel_centre() {
        let highs: Vec<f64> = (0..20).map(|index| 10.0 + index as f64).collect();
        let lows: Vec<f64> = (0..20).map(|index| index as f64).collect();
        let (lower, middle, upper) = donchian_channel(&highs, &lows, 3);
        assert!(close(middle[5], (upper[5] + lower[5]) / 2.0));
        // Indices 3,4,5 hold highs 13,14,15 and lows 3,4,5.
        assert!(close(upper[5], 15.0), "the highest of the last three highs");
        assert!(close(lower[5], 3.0), "the lowest of the last three lows");
    }

    /// VWAP weights by volume, so it sits nearer the heavier bar.
    #[test]
    fn vwap_weights_by_volume() {
        // Typical prices are 10 and 20; the second bar carries nine times the volume.
        let result = vwap(&[10.0, 20.0], &[10.0, 20.0], &[10.0, 20.0], &[1.0, 9.0], 2);
        assert!(close(result[1], 19.0), "got {}", result[1]);
    }

    /// A window with no volume is undefined rather than a made-up level.
    #[test]
    fn vwap_with_no_volume_is_undefined() {
        let result = vwap(&[10.0], &[10.0], &[10.0], &[0.0], 1);
        assert!(result[0].is_nan(), "no volume means no average price");
    }

    /// VWAP uses the typical price, not the close.
    #[test]
    fn vwap_uses_the_typical_price() {
        // A single bar: high 30, low 6, close 12 -> typical (30 + 6 + 12) / 3 = 16.
        let result = vwap(&[30.0], &[6.0], &[12.0], &[5.0], 1);
        assert!(close(result[0], 16.0), "got {}", result[0]);
    }

    /// MFI saturates when every bar rises with volume.
    #[test]
    fn money_flow_index_saturates_on_a_pure_uptrend() {
        let highs: Vec<f64> = (0..20).map(|index| 11.0 + index as f64).collect();
        let lows: Vec<f64> = (0..20).map(|index| 10.0 + index as f64).collect();
        let closes: Vec<f64> = (0..20).map(|index| 10.5 + index as f64).collect();
        let result = money_flow_index(&highs, &lows, &closes, &[100.0; 20], 5);
        assert!(close(result[19], 100.0), "every bar rose, so there is no negative flow");
    }

    /// MFI bottoms out when every bar falls.
    #[test]
    fn money_flow_index_bottoms_on_a_pure_downtrend() {
        let highs: Vec<f64> = (0..20).map(|index| 11.0 - index as f64).collect();
        let lows: Vec<f64> = (0..20).map(|index| 10.0 - index as f64).collect();
        let closes: Vec<f64> = (0..20).map(|index| 10.5 - index as f64).collect();
        let result = money_flow_index(&highs, &lows, &closes, &[100.0; 20], 5);
        assert!(close(result[19], 0.0), "no positive flow means the minimum reading");
    }

    /// A flat series is neither, which reads neutral.
    #[test]
    fn money_flow_index_of_a_flat_series_is_neutral() {
        let result = money_flow_index(&[5.0; 20], &[5.0; 20], &[5.0; 20], &[100.0; 20], 5);
        assert!(close(result[19], 50.0), "nothing moved in either direction");
    }

    /// MFI stays inside its published bounds.
    #[test]
    fn money_flow_index_stays_within_its_bounds() {
        let highs: Vec<f64> =
            (0..120).map(|index| 20.0 + (index as f64 * 0.4).sin() * 3.0).collect();
        let lows: Vec<f64> = highs.iter().map(|high| high - 1.0).collect();
        let closes: Vec<f64> = highs.iter().map(|high| high - 0.5).collect();
        let volumes: Vec<f64> = (0..120).map(|index| 100.0 + (index % 7) as f64 * 10.0).collect();
        for value in money_flow_index(&highs, &lows, &closes, &volumes, 14)
            .iter()
            .filter(|value| value.is_finite())
        {
            assert!((0.0..=100.0).contains(value), "MFI out of range: {value}");
        }
    }

    /// OBV adds on up bars and subtracts on down bars.
    #[test]
    fn on_balance_volume_accumulates_by_direction() {
        let result = on_balance_volume(&[10.0, 11.0, 10.0, 12.0], &[100.0, 200.0, 300.0, 400.0]);
        assert!(close(result[0], 100.0), "seeded with the first bar's volume");
        assert!(close(result[1], 300.0), "an up bar adds");
        assert!(close(result[2], 0.0), "a down bar subtracts");
        assert!(close(result[3], 400.0), "and it keeps accumulating");
    }

    /// An unchanged close leaves OBV alone, which is the definition.
    #[test]
    fn on_balance_volume_ignores_unchanged_bars() {
        let result = on_balance_volume(&[10.0, 10.0, 10.0], &[50.0, 50.0, 50.0]);
        assert!(result.iter().all(|value| close(*value, 50.0)), "nothing moved, nothing changed");
    }

    /// A gap produces a gap, because the next bar's direction is unknowable.
    ///
    /// The first version treated the bar after a gap as "unchanged" — comparing against
    /// `NAN` is false in both directions — which silently reported a flat market where the
    /// data was simply missing. This asserts the honest answer instead.
    #[test]
    fn on_balance_volume_reports_a_gap_rather_than_assuming_no_change() {
        let result = on_balance_volume(&[10.0, f64::NAN, 12.0], &[100.0, 200.0, 300.0]);
        assert!(close(result[0], 100.0), "the first bar seeds the total");
        assert!(result[1].is_nan(), "the missing bar has no value");
        assert!(result[2].is_nan(), "and neither has the bar whose direction needs it");
    }

    /// The total resumes once a sample and its predecessor are both real.
    #[test]
    fn on_balance_volume_resumes_after_a_gap() {
        let closes = [10.0, f64::NAN, 12.0, 13.0];
        let volumes = [100.0, 200.0, 300.0, 400.0];
        let result = on_balance_volume(&closes, &volumes);
        assert!(result[1].is_nan() && result[2].is_nan(), "the gap and its shadow");
        assert!(close(result[3], 500.0), "12 -> 13 is an up bar, so 100 + 400");
    }

    /// The extent ignores gaps and infinities.
    #[test]
    fn the_extent_ignores_gaps() {
        let (low, high) = series_extent(&[1.0, f64::NAN, 5.0, f64::INFINITY]);
        assert!(close(low, 1.0));
        assert!(close(high, 5.0), "infinity is a data error, not a level");
    }

    /// An all-gap series has no extent, which reads as nothing to draw.
    #[test]
    fn an_all_gap_series_has_no_extent() {
        let (low, high) = series_extent(&[f64::NAN, f64::INFINITY]);
        assert!(low.is_nan() && high.is_nan());
        assert!(!has_drawable_values(&[f64::NAN]));
        assert!(has_drawable_values(&[f64::NAN, 1.0]));
    }

    /// Left alignment puts an indicator's warm-up gap at the start, where it belongs.
    #[test]
    fn align_left_pads_at_the_start() {
        let aligned = align_left(&[1.0, 2.0, 3.0], 5);
        assert_eq!(aligned.len(), 5);
        assert!(aligned[0].is_nan() && aligned[1].is_nan(), "the gap is leading");
        assert!(close(aligned[2], 1.0) && close(aligned[4], 3.0));
    }

    /// A series at or beyond the window length is truncated rather than padded.
    #[test]
    fn align_left_truncates_a_longer_series() {
        let aligned = align_left(&[1.0, 2.0, 3.0, 4.0], 2);
        assert_eq!(aligned, vec![1.0, 2.0], "the leading part that fits");
    }

    /// Aligning an already-correct series is the identity, so the overlay path cannot
    /// shift data that needed no moving.
    #[test]
    fn align_left_is_the_identity_at_the_right_length() {
        let values = vec![1.0, 2.0, 3.0];
        assert_eq!(align_left(&values, 3), values);
    }

    /// Every indicator must preserve the input length, since the panes align by index.
    ///
    /// This is the one property the whole overlay design depends on: a series of the
    /// wrong length would draw at the wrong bars, and nothing else in the code would
    /// notice. Asserting it once for every function is cheaper than re-deriving the
    /// alignment at each call site.
    #[test]
    fn every_indicator_preserves_the_input_length() {
        let values: Vec<f64> = (0..40).map(|index| 10.0 + index as f64).collect();
        let volumes = vec![1.0; 40];
        assert_eq!(sma(&values, 5).len(), 40);
        assert_eq!(ema(&values, 5).len(), 40);
        assert_eq!(wilder_smooth(&values, 5).len(), 40);
        assert_eq!(rsi(&values, 5).len(), 40);
        let (line, signal, histogram) = macd(&values, 3, 6, 2);
        assert!(line.len() == 40 && signal.len() == 40 && histogram.len() == 40);
        let (lower, middle, upper) = bollinger_bands(&values, 5, 2.0);
        assert!(lower.len() == 40 && middle.len() == 40 && upper.len() == 40);
        let (k, d) = stochastic(&values, &values, &values, 5, 3, 3);
        assert!(k.len() == 40 && d.len() == 40);
        assert_eq!(atr(&values, &values, &values, 5).len(), 40);
        let (upper, lower) = trailing_extremes(&values, &values, 5);
        assert!(upper.len() == 40 && lower.len() == 40);
        let (lower, middle, upper) = donchian_channel(&values, &values, 5);
        assert!(lower.len() == 40 && middle.len() == 40 && upper.len() == 40);
        assert_eq!(vwap(&values, &values, &values, &volumes, 5).len(), 40);
        assert_eq!(money_flow_index(&values, &values, &values, &volumes, 5).len(), 40);
        assert_eq!(on_balance_volume(&values, &volumes).len(), 40);
    }
}
