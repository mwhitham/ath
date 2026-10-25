//! Read the record messages in a FIT activity file.

#[derive(Debug, Clone)]
pub struct FitPoint {
    pub offset_ms: i64,
    pub bpm: Option<f64>,
    pub latitude_deg: Option<f64>,
    pub longitude_deg: Option<f64>,
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

pub fn read_fit(bytes: &[u8]) -> Result<Vec<FitPoint>, String> {
    if bytes.len() < 12 {
        return Err("FIT file is too short".into());
    }
    let header_len = bytes[0] as usize;
    if bytes.len() < header_len + 4 || &bytes[8..12] != b".FIT" {
        return Err("FIT header is not readable".into());
    }
    let data_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let data_end = header_len + data_size;
    if bytes.len() < data_end {
        return Err("FIT data is shorter than the header says".into());
    }
    let mut cursor = header_len;
    let mut definitions = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];
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
                fields.push(FieldDef {
                    number: bytes[cursor],
                    size: bytes[cursor + 1],
                    base_type: bytes[cursor + 2],
                });
                cursor += 3;
            }
            definitions[local] = Some(Definition {
                global,
                little_endian: architecture == 0,
                fields,
            });
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
            let mut point = FitPoint {
                offset_ms: 0,
                bpm: None,
                latitude_deg: None,
                longitude_deg: None,
                recorded_at_ms: 0,
            };
            for (number, value) in values {
                match number {
                    253 => point.recorded_at_ms = fit_timestamp_ms(value),
                    0 => point.latitude_deg = semicircles_to_degrees(value),
                    1 => point.longitude_deg = semicircles_to_degrees(value),
                    3 => point.bpm = value,
                    _ => {}
                }
            }
            if start_ms.is_none() {
                start_ms = Some(point.recorded_at_ms);
            }
            point.offset_ms = point.recorded_at_ms - start_ms.unwrap_or(point.recorded_at_ms);
            points.push(point);
        }
    }
    Ok(points)
}

fn decode(bytes: &[u8], base_type: u8, little_endian: bool) -> Option<f64> {
    let base = base_type & 0x1f;
    if !little_endian {
        return None;
    }
    match (base, bytes.len()) {
        (0 | 1 | 2 | 10, 1) => Some(bytes[0] as f64),
        (4 | 6 | 13, 4) => Some(u32::from_le_bytes(bytes.try_into().ok()?) as f64),
        (3 | 5, 2) => Some(u16::from_le_bytes(bytes.try_into().ok()?) as f64),
        _ if bytes.len() == 4 => Some(u32::from_le_bytes(bytes.try_into().ok()?) as f64),
        _ if bytes.len() == 2 => Some(u16::from_le_bytes(bytes.try_into().ok()?) as f64),
        _ if bytes.len() == 1 => Some(bytes[0] as f64),
        _ => None,
    }
}

fn fit_timestamp_ms(seconds: Option<f64>) -> i64 {
    const FIT_EPOCH_UNIX: i64 = 631_065_600;
    (FIT_EPOCH_UNIX + seconds.unwrap_or(0.0) as i64) * 1000
}

fn semicircles_to_degrees(value: Option<f64>) -> Option<f64> {
    let raw = value? as u32 as i32;
    Some(raw as f64 * (180.0 / 2_147_483_648.0))
}

#[cfg(test)]
mod tests {
    use super::read_fit;

    #[test]
    fn reads_one_record_message() {
        let mut bytes = vec![14, 0x10, 0, 0, 0, 0, 0, 0, b'.', b'F', b'I', b'T', 0, 0];
        // definition for local 0, message 20, little endian, three fields
        bytes.extend_from_slice(&[0x40, 0, 0, 20, 0, 3]);
        bytes.extend_from_slice(&[253, 4, 0x86]); // timestamp uint32
        bytes.extend_from_slice(&[0, 4, 0x85]); // lat sint32 stored as 4 bytes
        bytes.extend_from_slice(&[3, 1, 2]); // hr uint8
        // data
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
    }
}
