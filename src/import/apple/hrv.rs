//! RMSSD from beat-to-beat intervals.
//!
//! Apple reports heart rate variability only as SDNN. WHOOP and Oura report
//! RMSSD. The two are different statistics and never share a baseline (D22).
//! Apple's export carries the beats under its SDNN value, and an ECG carries
//! the waveform, so RMSSD is calculated from those beats (D26). The SDNN value
//! stays an `hrv_sdnn` sample.
//!
//! Two calculations ran in 2.1.0, and both are kept with their own rounding:
//!
//! - `rmssd_from_beats` ran in the Rust helper for the beats under an SDNN
//!   record. It splits the beats where one was missed and rounds with
//!   `floor(x + 0.5)`.
//! - `rmssd_from_intervals` ran in JavaScript for ECG beats. It rounds with
//!   `Math.round`.

use crate::model::grade::js_round;

/// Shortest interval treated as a real heartbeat: 300 ms is 200 bpm.
const MIN_PLAUSIBLE_INTERVAL_MS: f64 = 300.0;
/// Longest interval treated as a real heartbeat: 2000 ms is 30 bpm.
const MAX_PLAUSIBLE_INTERVAL_MS: f64 = 2000.0;
/// Below this many usable ECG intervals the result is noise.
pub const MIN_INTERVALS_FOR_RMSSD: usize = 20;
/// Shortest window worth calculating over, in seconds.
pub const MIN_WINDOW_S_FOR_RMSSD: f64 = 10.0;
/// Fewest successive pairs worth calculating over.
pub const MIN_PAIRS_FOR_RMSSD: usize = 10;
/// A gap wider than this multiple of the beat's interval means a missed beat.
const GAP_TOLERANCE: f64 = 1.5;

/// An RMSSD value with how it was calculated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rmssd {
    pub rmssd_ms: f64,
    pub n_beats: usize,
    pub n_dropped: usize,
    pub window_s: f64,
}

/// One beat: when it landed in its window, and the interval it reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beat {
    pub offset_ms: i64,
    pub interval_ms: f64,
}

fn plausible(interval_ms: f64) -> bool {
    (MIN_PLAUSIBLE_INTERVAL_MS..=MAX_PLAUSIBLE_INTERVAL_MS).contains(&interval_ms)
}

/// The 2.1.0 helper's rounding: `floor(x + 0.5)`. It differs from
/// `Math.round` only next to a half.
pub fn helper_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    (value + 0.5).floor()
}

/// RMSSD over intervals, as ECG beats were calculated. Nothing when fewer
/// than 20 plausible intervals or less than 10 seconds remain.
pub fn rmssd_from_intervals(intervals_ms: &[f64]) -> Option<Rmssd> {
    let usable: Vec<f64> = intervals_ms.iter().copied().filter(|ms| plausible(*ms)).collect();
    let n_dropped = intervals_ms.len() - usable.len();
    if usable.len() < MIN_INTERVALS_FOR_RMSSD {
        return None;
    }
    let window_s = usable.iter().fold(0.0, |sum, ms| sum + ms) / 1000.0;
    if window_s < MIN_WINDOW_S_FOR_RMSSD {
        return None;
    }
    let mut sum_squares = 0.0;
    for pair in usable.windows(2) {
        let diff = pair[1] - pair[0];
        sum_squares += diff * diff;
    }
    let rmssd = (sum_squares / (usable.len() - 1) as f64).sqrt();
    Some(Rmssd {
        rmssd_ms: js_round(rmssd * 10.0) / 10.0,
        n_beats: usable.len(),
        n_dropped,
        window_s: js_round(window_s * 10.0) / 10.0,
    })
}

/// RMSSD over timestamped beats, skipping pairs that span a missed beat. A
/// dropped beat would otherwise read as real variation.
pub fn rmssd_from_beats(beats: &[Beat]) -> Option<Rmssd> {
    let plausible: Vec<&Beat> = beats.iter().filter(|beat| plausible(beat.interval_ms)).collect();
    let n_dropped = beats.len() - plausible.len();
    if plausible.len() < 2 {
        return None;
    }
    let mut sum_squares = 0.0;
    let mut pairs = 0_usize;
    for pair in plausible.windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        let observed = current.offset_ms - previous.offset_ms;
        let continuous = observed > 0 && (observed as f64) <= current.interval_ms * GAP_TOLERANCE;
        if continuous {
            let diff = current.interval_ms - previous.interval_ms;
            sum_squares += diff * diff;
            pairs += 1;
        }
    }
    if pairs < MIN_PAIRS_FOR_RMSSD {
        return None;
    }
    let span_ms = plausible[plausible.len() - 1].offset_ms - plausible[0].offset_ms;
    let window_s = span_ms as f64 / 1000.0;
    if window_s < MIN_WINDOW_S_FOR_RMSSD {
        return None;
    }
    Some(Rmssd {
        rmssd_ms: helper_round((sum_squares / pairs as f64).sqrt() * 10.0) / 10.0,
        n_beats: plausible.len(),
        n_dropped,
        window_s: helper_round(window_s * 10.0) / 10.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeat_to(pattern: &[f64], length: usize) -> Vec<f64> {
        pattern.iter().copied().cycle().take(length).collect()
    }

    fn continuous(intervals: &[f64]) -> Vec<Beat> {
        let mut offset = 0.0;
        intervals
            .iter()
            .map(|interval| {
                let beat = Beat { offset_ms: offset as i64, interval_ms: *interval };
                offset += interval;
                beat
            })
            .collect()
    }

    #[test]
    fn intervals_match_a_hand_calculated_value() {
        let result = rmssd_from_intervals(&repeat_to(&[800.0, 900.0], 40)).unwrap();
        assert_eq!(result, Rmssd { rmssd_ms: 100.0, n_beats: 40, n_dropped: 0, window_s: 34.0 });
        assert_eq!(rmssd_from_intervals(&repeat_to(&[850.0], 40)).unwrap().rmssd_ms, 0.0);
    }

    #[test]
    fn implausible_intervals_are_dropped_and_counted() {
        let mut intervals = repeat_to(&[800.0, 900.0], 40);
        intervals.extend([2500.0, 120.0]);
        let result = rmssd_from_intervals(&intervals).unwrap();
        assert_eq!((result.n_dropped, result.n_beats, result.rmssd_ms), (2, 40, 100.0));
    }

    #[test]
    fn too_little_data_publishes_nothing() {
        assert_eq!(rmssd_from_intervals(&repeat_to(&[850.0], MIN_INTERVALS_FOR_RMSSD - 1)), None);
        // 25 beats at 350 ms is only 8.75 seconds.
        assert_eq!(rmssd_from_intervals(&repeat_to(&[350.0], 25)), None);
        assert_eq!(rmssd_from_intervals(&repeat_to(&[50.0], 40)), None);
    }

    #[test]
    fn beats_agree_with_intervals_when_none_are_missing() {
        let result = rmssd_from_beats(&continuous(&repeat_to(&[800.0, 900.0], 40))).unwrap();
        assert_eq!((result.rmssd_ms, result.n_beats), (100.0, 40));
    }

    #[test]
    fn the_pair_across_a_missed_beat_is_ignored() {
        let mut beats = continuous(&repeat_to(&[800.0], 40));
        beats.remove(20);
        let result = rmssd_from_beats(&beats).unwrap();
        assert_eq!((result.rmssd_ms, result.n_beats), (0.0, 39));
        let all = continuous(&repeat_to(&[700.0, 900.0], 60));
        let mut gappy = all.clone();
        gappy.remove(30);
        let (clean, gap) = (rmssd_from_beats(&all).unwrap(), rmssd_from_beats(&gappy).unwrap());
        assert!((gap.rmssd_ms - clean.rmssd_ms).abs() < 5.0);
    }

    #[test]
    fn isolated_beats_and_short_windows_publish_nothing() {
        let mut beats = continuous(&repeat_to(&[800.0, 900.0], 40));
        beats.push(Beat { offset_ms: 40_000, interval_ms: 2500.0 });
        assert_eq!(rmssd_from_beats(&beats).unwrap().n_dropped, 1);
        let isolated: Vec<Beat> = (0..40).map(|index| Beat { offset_ms: index * 5000, interval_ms: 800.0 }).collect();
        assert_eq!(rmssd_from_beats(&isolated), None);
        let ten_seconds = rmssd_from_beats(&continuous(&repeat_to(&[800.0, 900.0], 13))).unwrap();
        assert!(ten_seconds.window_s >= 10.0 && ten_seconds.window_s < 30.0);
    }

    #[test]
    fn the_two_roundings_differ_only_next_to_a_half() {
        assert_eq!(helper_round(2.5), 3.0);
        assert_eq!(helper_round(-2.5), -2.0);
        // 0.49999999999999994 + 0.5 rounds up to 1 before the floor.
        assert_eq!(helper_round(0.49999999999999994), 1.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
    }
}
