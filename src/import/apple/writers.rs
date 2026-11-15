//! Who wrote each Apple value, and on which device.
//!
//! An Apple Health export is a container. One export holds values from the
//! watch, the phone, a scale, and any app that writes into Health. Each writer
//! is its own source (D45), because a baseline pooled across a watch and a
//! scale describes neither. A writer named `Health` or `Manual` is the Health
//! app itself: a person typed the value, so it belongs to the manual source.
//!
//! A source is not split by device. A replaced watch that keeps its name
//! stays one source and lists both devices, each with the days it was seen
//! (D51). The device text's memory address and software version change on
//! their own, so they are not kept.
//!
//! These are the rules of `src/import/apple.ts` and `src/import/merge.ts` in
//! 2.1.0.

use std::collections::HashMap;

use crate::text::{is_js_whitespace, js_trim};

use super::super::provider::CandidateSink;

/// The parts of Apple's device text that describe the physical device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Device {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub hardware: Option<String>,
}

/// Field names Apple writes inside a `device` attribute.
const DEVICE_FIELDS: [&str; 8] =
    ["name", "manufacturer", "model", "hardware", "software", "firmware", "localIdentifier", "UDI"];

/// Which whitespace a device reader skips: JavaScript's, for the reader that
/// kept the device, or ASCII, for the helper's reader that chose a night's
/// device text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spaces {
    JavaScript,
    Ascii,
}

impl Spaces {
    fn is_space(self, character: char) -> bool {
        match self {
            Spaces::JavaScript => is_js_whitespace(character),
            Spaces::Ascii => character.is_ascii_whitespace(),
        }
    }
}

/// `<<HKDevice: 0x2809b6800>, name:Apple Watch, manufacturer:Apple Inc.,
/// model:Watch, hardware:Watch6,2, software:10.2>`. A value can hold a comma,
/// as `Watch6,2` does, so the text is cut at field names, not at commas.
pub fn parse_device(raw: &str, spaces: Spaces) -> Option<Device> {
    if raw.is_empty() {
        return None;
    }
    let mut body = raw;
    if let Some(rest) = raw.strip_prefix("<<HKDevice:") {
        if let Some(close) = rest.find('>') {
            let after = rest[close + 1..].strip_prefix(',').unwrap_or(&rest[close + 1..]);
            body = after.trim_start_matches(|c| spaces.is_space(c));
        }
    }
    let trimmed = match spaces {
        Spaces::JavaScript => body.trim_end_matches(is_js_whitespace),
        Spaces::Ascii => body.trim_end(),
    };
    if let Some(stripped) = trimmed.strip_suffix('>') {
        body = stripped;
    }
    // `(?:^|,\s*)(name|manufacturer|...):`
    let field_at = |at: usize| {
        DEVICE_FIELDS.iter().find_map(|field| {
            (body[at..].starts_with(field) && body.as_bytes().get(at + field.len()) == Some(&b':'))
                .then(|| (*field, at + field.len() + 1))
        })
    };
    let mut hits: Vec<(usize, &str, usize)> = Vec::new();
    if let Some((field, value_start)) = field_at(0) {
        hits.push((0, field, value_start));
    }
    for (index, _) in body.match_indices(',') {
        let after = &body[index + 1..];
        let cursor = body.len() - after.trim_start_matches(|c| spaces.is_space(c)).len();
        if let Some((field, value_start)) = field_at(cursor) {
            hits.push((index, field, value_start));
        }
    }
    let mut device = Device::default();
    for (index, (_, field, value_start)) in hits.iter().enumerate() {
        let end = hits.get(index + 1).map_or(body.len(), |next| next.0);
        let value = &body[*value_start..end];
        let value = match spaces {
            Spaces::JavaScript => js_trim(value),
            Spaces::Ascii => value.trim(),
        };
        if value.is_empty() {
            continue;
        }
        let slot = match *field {
            "name" => &mut device.name,
            "manufacturer" => &mut device.manufacturer,
            "model" => &mut device.model,
            "hardware" => &mut device.hardware,
            _ => continue,
        };
        *slot = Some(value.to_string());
    }
    (device != Device::default()).then_some(device)
}

/// A writer's name as an ID stem. A leading possessive is dropped: "Alex's
/// Apple Watch" and "Apple Watch" name the same kind of thing.
pub fn writer_slug(writer: &str) -> String {
    let first_end = writer.find(is_js_whitespace).unwrap_or(writer.len());
    let first = &writer[..first_end];
    let possessive =
        ["'s", "\u{2019}s"].iter().any(|ending| first.strip_suffix(ending).is_some_and(|owner| !owner.is_empty()));
    let stripped = if possessive && first_end < writer.len() {
        writer[first_end..].trim_start_matches(is_js_whitespace)
    } else {
        writer
    };
    let mut slug = String::new();
    let mut dash = false;
    for character in stripped.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "unknown".into()
    } else {
        slug.to_string()
    }
}

/// The vendor slug for a writer: the manufacturer's first word when the
/// device names one; Apple for iPhone, iPad, and Apple names; otherwise the
/// writer's first word.
pub fn vendor_of(writer: &str, device: Option<&Device>) -> String {
    let first = |slug: String| slug.split('-').next().unwrap_or("unknown").to_string();
    if let Some(manufacturer) = device.and_then(|device| device.manufacturer.as_deref()) {
        return first(writer_slug(manufacturer));
    }
    let slug = writer_slug(writer);
    for name in ["iphone", "ipad", "apple"] {
        if slug == name || slug.starts_with(&format!("{name}-")) {
            return "apple".into();
        }
    }
    first(slug)
}

/// Writers that are the Health app itself.
pub fn hand_entered(writer: &str) -> bool {
    matches!(writer.to_lowercase().as_str(), "health" | "manual")
}

/// One device under one source, with the days it was seen.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceSeen {
    pub source_id: String,
    pub device: Device,
    pub from: String,
    pub to: String,
}

/// A resolved writer: its source, and its device when the text names one.
#[derive(Debug, Clone)]
pub struct Writer {
    pub source_id: String,
    pub manual: bool,
    pub device_key: Option<(String, Device)>,
}

/// Sources for the writers met while reading, resolved once per distinct
/// writer and device text, and the devices seen under each source.
#[derive(Default)]
pub struct Writers {
    cache: HashMap<(String, String), Writer>,
    devices: HashMap<String, Option<Device>>,
    seen: Vec<DeviceSeen>,
    seen_index: HashMap<(String, Device), usize>,
    accepted: HashMap<(String, Device), u64>,
}

impl Writers {
    fn device(&mut self, raw: &str) -> Option<Device> {
        self.devices.entry(raw.to_string()).or_insert_with(|| parse_device(raw, Spaces::JavaScript)).clone()
    }

    /// The source for a writer and device text, noting the device on a day.
    pub fn resolve(&mut self, sink: &mut CandidateSink<'_>, name: &str, device_raw: &str, on_day: &str) -> Writer {
        let trimmed = js_trim(name);
        let writer = if trimmed.is_empty() { "Unknown" } else { trimmed };
        let key = (writer.to_string(), device_raw.to_string());
        let hit = match self.cache.get(&key) {
            Some(hit) => hit.clone(),
            None => {
                let hit = if hand_entered(writer) {
                    Writer { source_id: sink.manual_source(), manual: true, device_key: None }
                } else {
                    let device = self.device(device_raw);
                    let source_id = sink.export_source(writer, &vendor_of(writer, device.as_ref()), None);
                    Writer { device_key: device.map(|device| (source_id.clone(), device)), source_id, manual: false }
                };
                self.cache.insert(key, hit.clone());
                hit
            }
        };
        if let Some(device_key) = &hit.device_key {
            if super::xml::is_calendar_day(on_day) {
                self.observe(device_key, on_day);
            }
        }
        hit
    }

    fn observe(&mut self, key: &(String, Device), on_day: &str) {
        match self.seen_index.get(key) {
            Some(index) => {
                let seen = &mut self.seen[*index];
                if on_day < seen.from.as_str() {
                    seen.from = on_day.to_string();
                }
                if on_day > seen.to.as_str() {
                    seen.to = on_day.to_string();
                }
            }
            None => {
                self.seen_index.insert(key.clone(), self.seen.len());
                self.seen.push(DeviceSeen {
                    source_id: key.0.clone(),
                    device: key.1.clone(),
                    from: on_day.to_string(),
                    to: on_day.to_string(),
                });
            }
        }
    }

    /// Count one stored record under a device.
    pub fn accept(&mut self, key: &(String, Device)) {
        *self.accepted.entry(key.clone()).or_default() += 1;
    }

    /// Devices in the order they were first seen, with their record counts.
    pub fn devices_seen(&self) -> impl Iterator<Item = (&DeviceSeen, u64)> {
        self.seen.iter().map(|seen| {
            let count = self.accepted.get(&(seen.source_id.clone(), seen.device.clone())).copied().unwrap_or_default();
            (seen, count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATCH: &str = "<<HKDevice: 0x2809b6800>, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch6,2, software:10.2>";

    fn device(name: &str, manufacturer: &str, model: &str, hardware: &str) -> Device {
        let some = |text: &str| (!text.is_empty()).then(|| text.to_string());
        Device { name: some(name), manufacturer: some(manufacturer), model: some(model), hardware: some(hardware) }
    }

    #[test]
    fn keeps_the_physical_device_and_the_comma_in_hardware() {
        assert_eq!(
            parse_device(WATCH, Spaces::JavaScript),
            Some(device("Apple Watch", "Apple Inc.", "Watch", "Watch6,2"))
        );
        assert_eq!(parse_device(WATCH, Spaces::Ascii), Some(device("Apple Watch", "Apple Inc.", "Watch", "Watch6,2")));
        assert_eq!(
            parse_device("name:iPhone, manufacturer:Apple Inc., model:iPhone, hardware:iPhone15,2", Spaces::JavaScript),
            Some(device("iPhone", "Apple Inc.", "iPhone", "iPhone15,2"))
        );
    }

    #[test]
    fn invents_no_device() {
        assert_eq!(parse_device("", Spaces::JavaScript), None);
        assert_eq!(parse_device("<<HKDevice: 0x1>, software:10.2>", Spaces::JavaScript), None);
        assert_eq!(parse_device("<<HKDevice: 0x1>, name:, software:1>", Spaces::JavaScript), None);
    }

    #[test]
    fn slugs_drop_a_possessive_and_vendors_follow_the_manufacturer() {
        assert_eq!(writer_slug("Alex's Apple Watch"), "apple-watch");
        assert_eq!(writer_slug("Alex\u{2019}s iPhone"), "iphone");
        assert_eq!(writer_slug("'s Watch"), "s-watch");
        assert_eq!(writer_slug("WHOOP"), "whoop");
        assert_eq!(writer_slug("!!!"), "unknown");
        assert_eq!(vendor_of("Alex's Apple Watch", Some(&device("", "Apple Inc.", "", ""))), "apple");
        assert_eq!(vendor_of("iPhone", None), "apple");
        assert_eq!(vendor_of("Applesauce", None), "applesauce");
        assert_eq!(vendor_of("Withings", None), "withings");
        assert_eq!(vendor_of("WHOOP", None), "whoop");
    }

    #[test]
    fn hand_entered_writers_are_the_health_app() {
        assert!(hand_entered("Health"));
        assert!(hand_entered("MANUAL"));
        assert!(!hand_entered("Health Mate"));
    }
}
