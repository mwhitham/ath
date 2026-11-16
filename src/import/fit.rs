//! The record messages of a FIT activity file.
//!
//! Only record messages (global message 20) are read: their time, position,
//! and heart rate. FIT marks an absent value with one reserved value per base
//! type, such as `0x7FFFFFFF` for a 32-bit signed integer. A reserved value is
//! absent, not a coordinate or a heart rate. A record message without a time
//! is left out, because a missing time is not the FIT epoch.
//!
//! This is the decoder that ran in 2.1.0 (`crates/ath-core/src/fit.rs`).
//! Compressed timestamp headers are refused with a message.

/// One record message with a time.
#[derive(Debug, Clone, PartialEq)]
pub struct FitPoint {
    /// Milliseconds since the first timed record message.
    pub offset_ms: i64,
    pub bpm: Option<f64>,
    pub latitude_deg: Option<f64>,
    pub longitude_deg: Option<f64>,
    /// Milliseconds since 1970.
    pub recorded_at_ms: i64,
}

#[derive(Debug)]
struct FieldDef {
    number: u8,
    size: u8,
    base_type: u8,
}

#[derive(Debug)]
struct Definition {
    global: u16,
    little_endian: bool,
    fields: Vec<FieldDef>,
}

/// The timed record messages of a FIT file, in file order. The error text is
/// what 2.1.0 reported for the file.
pub fn read_fit(bytes: &[u8]) -> Result<Vec<FitPoint>, String> {
    if bytes.len() < 12 {
        return Err("FIT file is too short".into());
    }
    let header_len = bytes[0] as usize;
    if bytes.len() < header_len + 4 || &bytes[8..12] != b".FIT" {
        return Err("FIT header is not readable".into());
    }
    let data_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let data_end = header_len + data_size;
    if bytes.len() < data_end {
        return Err("FIT data is shorter than the header says".into());
    }
    let mut cursor = header_len;
    let mut definitions: [Option<Definition>; 16] = Default::default();
    let mut points = Vec::new();
    let mut start_ms: Option<i64> = None;
    while cursor < data_end {
        let header = bytes[cursor];
        cursor += 1;
        if header & 0x80 != 0 {
            return Err("compressed FIT timestamps are left unread".into());
        }
        let local = (header & 0x0f) as usize;
        if header & 0x40 != 0 {
            if cursor + 5 > data_end {
                return Err("FIT definition is cut off".into());
            }
            let architecture = bytes[cursor + 1];
            let global = if architecture == 0 {
                u16::from_le_bytes([bytes[cursor + 2], bytes[cursor + 3]])
            } else {
                u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]])
            };
            let count = bytes[cursor + 4] as usize;
            cursor += 5;
            if cursor + count * 3 > data_end {
                return Err("FIT field list is cut off".into());
            }
            let mut fields = Vec::with_capacity(count);
            for _ in 0..count {
                fields.push(FieldDef { number: bytes[cursor], size: bytes[cursor + 1], base_type: bytes[cursor + 2] });
                cursor += 3;
            }
            definitions[local] = Some(Definition { global, little_endian: architecture == 0, fields });
            continue;
        }
        let Some(definition) = definitions[local].as_ref() else {
            return Err("FIT data message has no definition".into());
        };
        let mut values = Vec::new();
        for field in &definition.fields {
            if cursor + field.size as usize > data_end {
                return Err("FIT field is cut off".into());
            }
            let raw = &bytes[cursor..cursor + field.size as usize];
            cursor += field.size as usize;
            values.push((field.number, decode(raw, field.base_type, definition.little_endian)));
        }
        if definition.global == 20 {
            let mut point =
                FitPoint { offset_ms: 0, bpm: None, latitude_deg: None, longitude_deg: None, recorded_at_ms: 0 };
            let mut has_time = false;
            for (number, value) in values {
                match number {
                    253 => {
                        if let Some(seconds) = value {
                            point.recorded_at_ms = fit_timestamp_ms(seconds);
                            has_time = true;
                        }
                    }
                    0 => point.latitude_deg = semicircles_to_degrees(value),
                    1 => point.longitude_deg = semicircles_to_degrees(value),
                    3 => point.bpm = value,
                    _ => {}
                }
            }
            if !has_time {
                continue;
            }
            let start = *start_ms.get_or_insert(point.recorded_at_ms);
            point.offset_ms = point.recorded_at_ms - start;
            points.push(point);
        }
    }
    Ok(points)
}

fn decode(bytes: &[u8], base_type: u8, little_endian: bool) -> Option<f64> {
    let base = base_type & 0x1f;
    let value = match (base, bytes.len()) {
        (1, 1) => i8::from_le_bytes([bytes[0]]) as f64,
        (0 | 2 | 10 | 13, 1) => bytes[0] as f64,
        (3, 2) => i16::from_ne_bytes(ordered::<2>(bytes, little_endian)?) as f64,
        (4 | 11, 2) => u16::from_ne_bytes(ordered::<2>(bytes, little_endian)?) as f64,
        (5, 4) => i32::from_ne_bytes(ordered::<4>(bytes, little_endian)?) as f64,
        (6 | 12 | 8, 4) => u32::from_ne_bytes(ordered::<4>(bytes, little_endian)?) as f64,
        _ => return None,
    };
    if is_invalid(value, base) {
        return None;
    }
    Some(value)
}

/// The bytes of a field in this machine's byte order.
fn ordered<const N: usize>(bytes: &[u8], little_endian: bool) -> Option<[u8; N]> {
    let mut bytes: [u8; N] = bytes.try_into().ok()?;
    if little_endian != cfg!(target_endian = "little") {
        bytes.reverse();
    }
    Some(bytes)
}

/// FIT uses one reserved value per integer type for "not present". A latitude
/// of `0x7FFFFFFF` is that reserved value, not a coordinate.
fn is_invalid(value: f64, base: u8) -> bool {
    match base {
        0 | 2 | 13 => value == 255.0,
        1 => value == 127.0,
        3 => value == 32767.0,
        4 => value == 65535.0,
        5 => value == 2147483647.0,
        6 | 8 => value == 4294967295.0,
        10..=12 => value == 0.0,
        _ => false,
    }
}

/// FIT counts seconds from 1989-12-31T00:00:00Z.
fn fit_timestamp_ms(seconds: f64) -> i64 {
    const FIT_EPOCH_UNIX: i64 = 631_065_600;
    (FIT_EPOCH_UNIX + seconds as i64) * 1000
}

fn semicircles_to_degrees(value: Option<f64>) -> Option<f64> {
    Some(value? as i32 as f64 * (180.0 / 2_147_483_648.0))
}

#[cfg(test)]
mod tests {
    use super::read_fit;

    #[test]
    fn reads_one_record_message() {
        let mut bytes = vec![14, 0x10, 0, 0, 0, 0, 0, 0, b'.', b'F', b'I', b'T', 0, 0];
        // A definition for local message 0: global message 20, little endian, three fields.
        bytes.extend_from_slice(&[0x40, 0, 0, 20, 0, 3]);
        bytes.extend_from_slice(&[253, 4, 0x86]);
        bytes.extend_from_slice(&[0, 4, 0x85]);
        bytes.extend_from_slice(&[3, 1, 2]);
        bytes.push(0x00);
        bytes.extend_from_slice(&1_000_000_000u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.push(120);
        let data_size = (bytes.len() - 14) as u32;
        bytes[4..8].copy_from_slice(&data_size.to_le_bytes());
        let points = read_fit(&bytes).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].bpm, Some(120.0));
        assert_eq!(points[0].offset_ms, 0);
        assert_eq!(points[0].latitude_deg, Some(0.0));
    }

    #[test]
    fn a_reserved_position_is_absent_and_a_big_endian_time_is_not_the_fit_epoch() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/exports/garmin/synthetic-position.fit");
        let points = read_fit(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(points.len(), 2);
        assert!(points[0].latitude_deg.is_none());
        assert!(points[0].longitude_deg.is_none());
        assert_eq!(points[0].bpm, Some(120.0));
        assert_eq!(crate::model::time::format_utc_millis(points[0].recorded_at_ms), "2026-08-09T15:00:00.000Z");
        assert_eq!(points[1].latitude_deg, Some(0.0));
        assert_eq!(points[1].longitude_deg, Some(0.0));
        assert!(points[1].bpm.is_none());
        assert_eq!(points[1].offset_ms, 10_000);
    }

    #[test]
    fn short_and_unreadable_files_are_refused_with_the_2_1_0_text() {
        assert_eq!(read_fit(b"short").unwrap_err(), "FIT file is too short");
        assert_eq!(
            read_fit(&[12, 0, 0, 0, 0, 0, 0, 0, b'X', b'F', b'I', b'T', 0, 0, 0, 0]).unwrap_err(),
            "FIT header is not readable"
        );
        let mut compressed = vec![12, 0x10, 0, 0, 1, 0, 0, 0, b'.', b'F', b'I', b'T', 0x80];
        compressed.extend_from_slice(&[0, 0, 0]);
        assert_eq!(read_fit(&compressed).unwrap_err(), "compressed FIT timestamps are left unread");
    }
}
