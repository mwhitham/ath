//! Apple Watch ECG recordings, read for beat timing rather than diagnosis.
//!
//! Each file in `electrocardiograms/` holds about 30 seconds of single-lead
//! ECG at about 512 samples a second. Its R peaks give beat timing measured
//! electrically, the reference standard for heart rate variability.
//!
//! The waveform and the rhythm classification are not stored. The
//! classification decides whether the recording is usable: in atrial
//! fibrillation the rhythm is irregular by definition, so RMSSD would measure
//! the arrhythmia rather than recovery. This is a method gate, not a diagnosis.
//!
//! The reading rules are 2.1.0's `src/import/ecg.ts`: header labels are found
//! wherever they appear, a quoted value may hold a comma, the sample rate is
//! the decimal Apple writes, and waveform lines are read with `Number()`.

use crate::cli::text::{is_js_whitespace, js_number, js_trim};

use super::rpeaks::detect_r_peaks;

/// A readable recording.
#[derive(Debug, Clone, PartialEq)]
pub struct EcgRecording {
    /// When the recording started, ISO 8601 with its offset.
    pub recorded_at: String,
    pub sample_rate_hz: f64,
    /// The rhythm classification, used as a gate and then dropped.
    pub classification: String,
    pub microvolts: Vec<f64>,
}

/// Why a recording could not be used, for the skip summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcgProblem {
    Unreadable(String),
    NotSinus(String),
    TooFewBeats(String),
}

/// Beat timing from a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct EcgBeats {
    pub recorded_at: String,
    /// Each beat's offset from the start of the recording, in milliseconds.
    pub offsets_ms: Vec<f64>,
    /// The interval before each beat after the first, in milliseconds.
    pub intervals_ms: Vec<f64>,
    pub quality: f64,
}

/// `/sinus\s*rhythm/i`: ASCII letters in either case, any spacing.
pub fn is_usable_rhythm(classification: &str) -> bool {
    let bytes = classification.as_bytes();
    let starts_with = |at: usize, word: &str| {
        bytes.len() >= at + word.len() && bytes[at..at + word.len()].eq_ignore_ascii_case(word.as_bytes())
    };
    (0..bytes.len()).any(|start| {
        if !starts_with(start, "sinus") {
            return false;
        }
        let rest = &classification[start + 5..];
        starts_with(classification.len() - rest.trim_start_matches(is_js_whitespace).len(), "rhythm")
    })
}

/// A metadata line as label and value. A quoted value may hold a comma.
fn split_metadata(line: &str) -> Option<(&str, &str)> {
    let comma = line.find(',')?;
    let label = js_trim(&line[..comma]);
    let mut value = js_trim(&line[comma + 1..]);
    if value.starts_with('"') {
        if let Some(closing) = value.rfind('"').filter(|closing| *closing > 0) {
            value = &value[1..closing];
        }
    }
    Some((label, value))
}

/// Apple writes the rate as `510.844 hertz`: the first run of digits and
/// points, with commas read as points.
fn parse_sample_rate(value: &str) -> Option<f64> {
    let value = value.replace(',', ".");
    let start = value.find(|c: char| c.is_ascii_digit() || c == '.')?;
    let run = &value[start..];
    let end = run.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(run.len());
    let hz = js_number(&run[..end]);
    (hz.is_finite() && hz > 0.0).then_some(hz)
}

fn digits(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_digit)
}

/// `2020-02-16 11:56:41 -0600` as ISO 8601 with its offset. Text after the
/// offset is ignored, as the 2.1.0 expression ignored it.
fn parse_recorded_date(value: &str) -> Option<String> {
    let text = js_trim(value);
    let bytes = text.as_bytes();
    let shaped = bytes.len() >= 19
        && digits(&bytes[0..4])
        && bytes[4] == b'-'
        && digits(&bytes[5..7])
        && bytes[7] == b'-'
        && digits(&bytes[8..10])
        && matches!(bytes[10], b' ' | b'T')
        && digits(&bytes[11..13])
        && bytes[13] == b':'
        && digits(&bytes[14..16])
        && bytes[16] == b':'
        && digits(&bytes[17..19]);
    if shaped {
        let rest = text[19..].trim_start_matches(is_js_whitespace).as_bytes();
        if rest.len() >= 5 && matches!(rest[0], b'+' | b'-') && digits(&rest[1..3]) {
            let minutes = if rest[3] == b':' { rest.get(4..6) } else { rest.get(3..5) };
            if let Some(minutes) = minutes.filter(|minutes| digits(minutes)) {
                return Some(format!(
                    "{}T{}{}{}:{}",
                    &text[0..10],
                    &text[11..19],
                    rest[0] as char,
                    std::str::from_utf8(&rest[1..3]).unwrap_or_default(),
                    std::str::from_utf8(minutes).unwrap_or_default()
                ));
            }
        }
    }
    is_full_iso(text).then(|| text.to_string())
}

/// `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$`.
pub fn is_full_iso(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || !digits(&bytes[0..4])
        || bytes[4] != b'-'
        || !digits(&bytes[5..7])
        || bytes[7] != b'-'
        || !digits(&bytes[8..10])
        || bytes[10] != b'T'
        || !digits(&bytes[11..13])
        || bytes[13] != b':'
        || !digits(&bytes[14..16])
        || bytes[16] != b':'
        || !digits(&bytes[17..19])
    {
        return false;
    }
    let mut index = 19;
    if bytes[index] == b'.' {
        let start = index + 1;
        index = start;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    let rest = &bytes[index..];
    rest == b"Z"
        || (rest.len() == 6
            && matches!(rest[0], b'+' | b'-')
            && digits(&rest[1..3])
            && rest[3] == b':'
            && digits(&rest[4..6]))
}

/// `/^-?[\d.]+$/`: a line that starts the waveform.
fn is_sample_line(line: &str) -> bool {
    let body = line.strip_prefix('-').unwrap_or(line);
    !body.is_empty() && body.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
}

/// Read one ECG CSV.
pub fn parse_ecg_csv(text: &str) -> Result<EcgRecording, EcgProblem> {
    let lines: Vec<&str> = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
    let mut recorded_at: Option<String> = None;
    let mut sample_rate_hz: Option<f64> = None;
    let mut classification = String::new();
    let mut first_sample = None;
    for (index, raw) in lines.iter().enumerate() {
        let line = js_trim(raw);
        if line.is_empty() {
            continue;
        }
        if is_sample_line(line) {
            first_sample = Some(index);
            break;
        }
        let Some((label, value)) = split_metadata(line) else {
            continue;
        };
        let label = label.to_lowercase();
        if label.contains("recorded date") {
            recorded_at = parse_recorded_date(value);
        } else if label.contains("sample rate") {
            sample_rate_hz = parse_sample_rate(value);
        } else if label.contains("classification") {
            classification = value.to_string();
        }
    }
    let Some(recorded_at) = recorded_at else {
        return Err(EcgProblem::Unreadable("no readable 'Recorded Date' in the header".into()));
    };
    let Some(sample_rate_hz) = sample_rate_hz else {
        return Err(EcgProblem::Unreadable("no readable 'Sample Rate' in the header".into()));
    };
    let Some(first_sample) = first_sample else {
        return Err(EcgProblem::Unreadable("header present but no waveform samples".into()));
    };
    let microvolts: Vec<f64> = lines[first_sample..]
        .iter()
        .map(|line| js_trim(line))
        .filter(|line| !line.is_empty())
        .map(js_number)
        .filter(|value| value.is_finite())
        .collect();
    if (microvolts.len() as f64) < sample_rate_hz * 5.0 {
        return Err(EcgProblem::Unreadable(format!("only {} samples, under 5 seconds of signal", microvolts.len())));
    }
    Ok(EcgRecording { recorded_at, sample_rate_hz, classification, microvolts })
}

/// Beat timing, or the reason there is none. The rhythm gate runs first, so
/// an arrhythmia never reaches a recovery baseline.
pub fn beats_from_ecg(recording: &EcgRecording) -> Result<EcgBeats, EcgProblem> {
    if !is_usable_rhythm(&recording.classification) {
        let shown = if recording.classification.is_empty() { "unknown" } else { &recording.classification };
        return Err(EcgProblem::NotSinus(format!(
            "classified \"{shown}\", where beat-to-beat variation does not describe recovery"
        )));
    }
    let found = detect_r_peaks(&recording.microvolts, recording.sample_rate_hz);
    if found.peaks.len() < 12 {
        return Err(EcgProblem::TooFewBeats(format!("{} beats detected", found.peaks.len())));
    }
    let ms_per_sample = 1000.0 / recording.sample_rate_hz;
    let offsets_ms: Vec<f64> = found.peaks.iter().map(|peak| *peak as f64 * ms_per_sample).collect();
    let intervals_ms = offsets_ms.windows(2).map(|pair| pair[1] - pair[0]).collect();
    Ok(EcgBeats { recorded_at: recording.recorded_at.clone(), offsets_ms, intervals_ms, quality: found.quality })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhythm_gate_accepts_only_sinus_rhythm() {
        assert!(is_usable_rhythm("Sinus Rhythm"));
        assert!(is_usable_rhythm("SINUSRHYTHM"));
        assert!(is_usable_rhythm("Sinus \u{a0}Rhythm, 70 bpm"));
        for other in ["Atrial Fibrillation", "Inconclusive", "High Heart Rate", "", "sinus"] {
            assert!(!is_usable_rhythm(other), "{other}");
        }
    }

    #[test]
    fn header_labels_and_values_read_as_2_1_0_read_them() {
        assert_eq!(split_metadata("Device,\"Watch7,2\""), Some(("Device", "Watch7,2")));
        assert_eq!(split_metadata("Name, \"open"), Some(("Name", "\"open")));
        assert_eq!(parse_sample_rate("510,844 hertz"), Some(510.844));
        assert_eq!(parse_sample_rate("1.2.3 hertz"), None);
        assert_eq!(parse_sample_rate("hertz"), None);
        assert_eq!(parse_recorded_date("2026-08-09 07:20:11 -0700").as_deref(), Some("2026-08-09T07:20:11-07:00"));
        assert_eq!(parse_recorded_date("2026-08-09T07:20:11+0530 extra").as_deref(), Some("2026-08-09T07:20:11+05:30"));
        assert_eq!(parse_recorded_date("2026-08-09T07:20:11.5Z").as_deref(), Some("2026-08-09T07:20:11.5Z"));
        assert_eq!(parse_recorded_date("9 Aug 2026"), None);
    }

    #[test]
    fn a_file_without_a_header_or_with_too_little_signal_is_unreadable() {
        assert_eq!(
            parse_ecg_csv("1.0\n2.0\n3.0\n"),
            Err(EcgProblem::Unreadable("no readable 'Recorded Date' in the header".into()))
        );
        let header = "Recorded Date,2026-08-09 07:20:11 -0700\nSample Rate,512 hertz\n\n";
        assert_eq!(
            parse_ecg_csv(&format!("{header}1.0\n2.0\n")),
            Err(EcgProblem::Unreadable("only 2 samples, under 5 seconds of signal".into()))
        );
    }

    #[test]
    fn waveform_lines_are_read_with_number() {
        let mut text = String::from("Recorded Date,2026-08-09 07:20:11 -0700\r\nSample Rate,0.5 hertz\r\n");
        text.push_str("1\r\n 0x10 \r\n1e1\r\nInfinity\r\nabc\r\n\r\n+5\r\n");
        let recording = parse_ecg_csv(&text).unwrap();
        assert_eq!(recording.microvolts, vec![1.0, 16.0, 10.0, 5.0]);
        assert_eq!(recording.classification, "");
    }
}
