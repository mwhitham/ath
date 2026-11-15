//! Finding heartbeats in an ECG waveform.
//!
//! This follows Pan and Tompkins (1985): band-pass the signal to where the QRS
//! complex lives, differentiate to find its steep slope, square so large slopes
//! dominate, integrate over about one QRS width, then take peaks above an
//! adaptive threshold with a refractory period after each.
//!
//! - Band-pass 5 to 15 Hz removes drift from breathing below and muscle noise
//!   above. A tall T wave sits mostly below the band.
//! - The derivative and square pick steepness rather than height, and an
//!   inverted lead still works.
//! - Integration over 150 ms turns the spikes within one beat into one bump.
//! - A 200 ms refractory period: the heart cannot beat again that soon.
//! - The threshold comes from percentiles, never the maximum. One motion
//!   artifact many times a beat's height would otherwise hide every beat.
//! - Each peak is placed on the band-passed signal, where integration has not
//!   smeared it, because interval timing is the point.
//!
//! The arithmetic is 2.1.0's `src/rpeaks.ts`, operation for operation, so the
//! same samples give the same beats.

use crate::model::grade::js_round;

const BANDPASS_LOW_HZ: f64 = 5.0;
const BANDPASS_HIGH_HZ: f64 = 15.0;
const INTEGRATION_WINDOW_S: f64 = 0.15;
const REFRACTORY_S: f64 = 0.2;
const PEAK_SEARCH_S: f64 = 0.1;

/// Detected beats and how far they stood above the background.
#[derive(Debug, Clone, PartialEq)]
pub struct RPeaks {
    /// Sample indices of the R peaks.
    pub peaks: Vec<usize>,
    /// The square root of the 95th over the 50th percentile, to one decimal.
    /// Noise does not stop detection; it degrades timing, and this says so.
    pub quality: f64,
}

/// A centred moving average, summed as 2.1.0 summed it.
fn moving_average(signal: &[f64], window_samples: f64) -> Vec<f64> {
    let width = js_round(window_samples).max(1.0);
    if width <= 1.0 {
        return signal.to_vec();
    }
    let half = (width / 2.0).floor() as usize;
    let mut out = vec![0.0; signal.len()];
    let mut sum = 0.0;
    let mut count = 0_usize;
    for value in signal.iter().take(half.min(signal.len())) {
        sum += value;
        count += 1;
    }
    for index in 0..signal.len() {
        let entering = index + half;
        if entering < signal.len() {
            sum += signal[entering];
            count += 1;
        }
        if index > half {
            sum -= signal[index - half - 1];
            count -= 1;
        }
        out[index] = if count > 0 { sum / count as f64 } else { 0.0 };
    }
    out
}

/// Subtract a slow moving average, then smooth.
fn bandpass(signal: &[f64], sample_rate_hz: f64) -> Vec<f64> {
    let drift = moving_average(signal, sample_rate_hz / BANDPASS_LOW_HZ);
    let high_passed: Vec<f64> = signal.iter().zip(&drift).map(|(value, drift)| value - drift).collect();
    moving_average(&high_passed, sample_rate_hz / BANDPASS_HIGH_HZ)
}

/// The five-point derivative Pan and Tompkins specify.
fn derivative(signal: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; signal.len()];
    if signal.len() > 4 {
        for index in 2..signal.len() - 2 {
            out[index] =
                (2.0 * signal[index + 2] + signal[index + 1] - signal[index - 1] - 2.0 * signal[index - 2]) / 8.0;
        }
    }
    out
}

/// The value at a fraction of sorted data, between neighbours.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor();
    let upper = position.ceil();
    if lower == upper {
        return sorted[lower as usize];
    }
    sorted[lower as usize] + (sorted[upper as usize] - sorted[lower as usize]) * (position - lower)
}

/// R peaks in a waveform. No peaks rather than a guess when the signal has no
/// clear rhythm, because a wrong beat list gives a confident wrong number.
pub fn detect_r_peaks(microvolts: &[f64], sample_rate_hz: f64) -> RPeaks {
    let empty = RPeaks { peaks: Vec::new(), quality: 0.0 };
    if (microvolts.len() as f64) < sample_rate_hz * 2.0 {
        return empty;
    }
    let filtered = bandpass(microvolts, sample_rate_hz);
    let squared: Vec<f64> = derivative(&filtered).into_iter().map(|value| value * value).collect();
    let integrated = moving_average(&squared, sample_rate_hz * INTEGRATION_WINDOW_S);

    let mut sorted: Vec<f64> = integrated.iter().copied().filter(|value| *value > 0.0).collect();
    if sorted.is_empty() {
        return empty;
    }
    sorted.sort_by(f64::total_cmp);
    let noise_level = percentile(&sorted, 0.5);
    let signal_level = percentile(&sorted, 0.95);
    if signal_level <= 0.0 {
        return empty;
    }
    let threshold = noise_level + 0.25 * (signal_level - noise_level);
    let quality = if noise_level > 0.0 { (signal_level / noise_level).sqrt() } else { 0.0 };

    let refractory = js_round(sample_rate_hz * REFRACTORY_S);
    let search = js_round(sample_rate_hz * PEAK_SEARCH_S).max(1.0) as usize;
    let mut peaks: Vec<usize> = Vec::new();
    let mut index = 1;
    while index + 1 < integrated.len() {
        if integrated[index] <= threshold {
            index += 1;
            continue;
        }
        let mut bump_end = index;
        while bump_end < integrated.len() && integrated[bump_end] > threshold {
            bump_end += 1;
        }
        let from = index.saturating_sub(search);
        let to = (filtered.len() - 1).min(bump_end + search);
        let mut best = from;
        let mut best_magnitude = filtered[from].abs();
        for (offset, value) in filtered[from + 1..=to].iter().enumerate() {
            let magnitude = value.abs();
            if magnitude > best_magnitude {
                best_magnitude = magnitude;
                best = from + 1 + offset;
            }
        }
        match peaks.last().copied() {
            Some(previous) if ((best as f64) - (previous as f64)) < refractory => {
                // The same beat found twice: keep the stronger candidate.
                if best_magnitude > filtered[previous].abs() {
                    *peaks.last_mut().expect("a previous peak") = best;
                }
            }
            _ => peaks.push(best),
        }
        index = bump_end + 1;
    }
    RPeaks { peaks, quality: js_round(quality * 10.0) / 10.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 510.844;

    /// A waveform with known beats, built only from exact arithmetic so the
    /// test does not depend on a platform's exponential.
    fn synthetic(intervals: &[f64]) -> Vec<f64> {
        let total_ms: f64 = intervals.iter().sum::<f64>() + 1000.0;
        let length = js_round(total_ms / 1000.0 * RATE) as usize;
        let mut signal = vec![0.0; length];
        let mut at_ms = 500.0;
        for interval in intervals {
            let centre = js_round(at_ms / 1000.0 * RATE) as i64;
            let width = js_round(0.04 * RATE) as i64;
            for offset in -width..=width {
                let index = centre + offset;
                if index < 0 || index >= length as i64 {
                    continue;
                }
                let x = offset as f64 / (0.02 * RATE);
                signal[index as usize] += 1000.0 / (1.0 + x * x * x * x);
            }
            at_ms += interval;
        }
        signal
    }

    #[test]
    fn finds_every_beat_in_a_steady_rhythm() {
        assert_eq!(detect_r_peaks(&synthetic(&[900.0; 30]), RATE).peaks.len(), 30);
    }

    #[test]
    fn survives_drift_and_a_large_artifact() {
        let mut drifting = synthetic(&[900.0; 30]);
        // A slow triangle wave stands in for breathing drift.
        let period = (RATE * 3.0) as usize;
        for (index, value) in drifting.iter_mut().enumerate() {
            let phase = (index % period) as f64 / period as f64;
            *value += 400.0 * (if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase });
        }
        assert_eq!(detect_r_peaks(&drifting, RATE).peaks.len(), 30);
        for amplitude in [5_000.0, 50_000.0, 200_000.0] {
            let mut signal = synthetic(&[900.0; 30]);
            let centre = js_round(5.0 * RATE) as usize;
            let half = js_round(0.05 * RATE) as usize;
            for (offset, value) in signal[centre - half..=centre + half].iter_mut().enumerate() {
                let x = (offset as f64 - half as f64) / (0.01 * RATE);
                *value += amplitude / (1.0 + x * x * x * x);
            }
            assert!(detect_r_peaks(&signal, RATE).peaks.len() >= 29, "{amplitude}");
        }
    }

    #[test]
    fn reports_nothing_without_a_rhythm_or_enough_signal() {
        assert!(detect_r_peaks(&vec![0.0; (RATE * 30.0) as usize], RATE).peaks.is_empty());
        assert!(detect_r_peaks(&[0.0; 100], RATE).peaks.is_empty());
    }

    #[test]
    fn moving_average_matches_the_centred_window() {
        assert_eq!(moving_average(&[1.0, 2.0, 3.0, 4.0], 3.0), vec![1.5, 2.0, 3.0, 3.5]);
        assert_eq!(moving_average(&[1.0, 2.0], 0.4), vec![1.0, 2.0]);
    }
}
