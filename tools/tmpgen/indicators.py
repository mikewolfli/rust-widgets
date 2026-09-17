import io
import os

HEADER = """// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
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
"""

SMA = '''
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
'''

EMA = '''
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
'''

WILDER = '''
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
'''

RSI = '''
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
'''

MACD = '''
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
'''

BOLL = '''
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
        let variance =
            window.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / period as f64;
        let deviation = variance.sqrt();
        lower[index] = mean - multiplier * deviation;
        upper[index] = mean + multiplier * deviation;
    }
    (lower, middle, upper)
}
'''

STOCH = '''
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
    for index in (period - 1)..length {
        let window_start = index + 1 - period;
        let window_high = &highs[window_start..=index];
        let window_low = &lows[window_start..=index];
        if !window_high.iter().all(|v| v.is_finite())
            || !window_low.iter().all(|v| v.is_finite())
            || !closes[index].is_finite()
        {
            continue;
        }
        let highest = window_high.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lowest = window_low.iter().copied().fold(f64::INFINITY, f64::min);
        let range = highest - lowest;
        raw_k[index] = if range == 0.0 { 50.0 } else { (closes[index] - lowest) / range * 100.0 };
    }

    let k = if k_smoothing <= 1 { raw_k } else { sma(&raw_k, k_smoothing) };
    let d = sma(&k, d_period);
    (k, d)
}
'''

TRUE_RANGE_ATR = '''
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
        ranges[index] = (high - low)
            .max((high - previous_close).abs())
            .max((low - previous_close).abs());
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
'''

EXTREMES = '''
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
'''

VWAP = '''
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
    for index in (period - 1)..length {
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
            out[index] = weighted / total_volume;
        }
    }
    out
}
'''

MFI = '''
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
            if total_positive == 0.0 { 50.0 } else { 100.0 }
        } else {
            let ratio = total_positive / total_negative;
            100.0 - (100.0 / (1.0 + ratio))
        };
    }
    out
}
'''

OBV = '''
/// On-Balance Volume: a running total that adds volume on up bars and subtracts it on
/// down bars.
///
/// Not padded — OBV is defined from its first sample, seeded with that bar's volume —
/// because a cumulative series has no warm-up window. A non-finite sample is skipped and
/// the total carries forward rather than resetting: the indicator counts what traded, and
/// a missing bar contributed nothing it can know about.
pub fn on_balance_volume(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let length = closes.len().min(volumes.len());
    let mut out = vec![f64::NAN; length];
    if length == 0 {
        return out;
    }
    let mut total = 0.0;
    for index in 0..length {
        if !closes[index].is_finite() || !volumes[index].is_finite() {
            out[index] = total;
            continue;
        }
        if index == 0 {
            total = volumes[index];
        } else if closes[index] > closes[index - 1] {
            total += volumes[index];
        } else if closes[index] < closes[index - 1] {
            total -= volumes[index];
        }
        out[index] = total;
    }
    out
}
'''

HELPERS = '''
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
'''

def main():
    out = io.StringIO()
    out.write(HEADER)
    for block in (SMA, EMA, WILDER, RSI, MACD, BOLL, STOCH, TRUE_RANGE_ATR,
                  EXTREMES, VWAP, MFI, OBV, HELPERS):
        out.write(block)
    text = out.getvalue()
    path = os.path.join("src", "widget", "special_widgets", "finance", "indicators.rs")
    with open(path, "w") as handle:
        handle.write(text)
    print("wrote", path, len(text.splitlines()), "lines")

main()
