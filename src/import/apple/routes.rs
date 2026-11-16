//! Workout routes in GPX files: points to store, and splits and climbing.
//!
//! An Apple export writes one GPX file per outdoor workout under
//! `workout-routes/`. A `WorkoutRoute` that names a file makes that file the
//! workout's stored route. Separately, every route file is summarized into
//! one split per kilometre and an elevation gain, and a workout whose time
//! overlaps the route takes them.
//!
//! 2.1.0 read GPX with regular expressions rather than an XML parser, and
//! these functions keep that reading exactly, including what it does with a
//! self-closing track point followed by an ordinary one. `Number()` and
//! `Date.parse` rules come from the JavaScript the old reader used. One
//! difference is deliberate: a time without a UTC offset is not read in the
//! computer's own timezone. A point with such a time is left out of the
//! summary, as it was already left out of the stored route.

use crate::model::*;
use crate::text::{js_number, js_trim};

use super::super::instant;
use super::trig;

/// One split per kilometre, as runners talk about a run.
const SPLIT_DISTANCE_M: f64 = 1000.0;
/// Elevation changes smaller than this between points are GPS noise.
const ELEVATION_NOISE_M: f64 = 1.0;
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Great-circle distance between two positions, with 2.1.0's arithmetic.
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let to_rad = std::f64::consts::PI / 180.0;
    let d_lat = (lat2 - lat1) * to_rad;
    let d_lon = (lon2 - lon1) * to_rad;
    let half_lat = trig::sin(d_lat / 2.0);
    let half_lon = trig::sin(d_lon / 2.0);
    let a = half_lat * half_lat + trig::cos(lat1 * to_rad) * trig::cos(lat2 * to_rad) * (half_lon * half_lon);
    2.0 * EARTH_RADIUS_M * trig::asin(a.sqrt())
}

/// One `<trkpt>` as the 2.1.0 expression
/// `/<trkpt\b([^>]*)>([\s\S]*?)<\/trkpt>|<trkpt\b([^>]*)\/>/g` found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackMatch<'a> {
    pub attributes: &'a str,
    pub body: &'a str,
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Every track point match in order.
pub fn track_points(text: &str) -> Vec<TrackMatch<'_>> {
    const OPEN: &str = "<trkpt";
    const CLOSE: &str = "</trkpt>";
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(OPEN) {
        let start = from + found;
        let after = start + OPEN.len();
        // `\b` after `t`: the next character is not a word character.
        if bytes.get(after).is_some_and(|byte| is_word_byte(*byte)) {
            from = start + 1;
            continue;
        }
        let Some(gt) = text[after..].find('>').map(|offset| after + offset) else {
            break;
        };
        if let Some(close) = text[gt + 1..].find(CLOSE).map(|offset| gt + 1 + offset) {
            out.push(TrackMatch { attributes: &text[after..gt], body: &text[gt + 1..close] });
            from = close + CLOSE.len();
            continue;
        }
        if gt > after && bytes[gt - 1] == b'/' {
            out.push(TrackMatch { attributes: &text[after..gt - 1], body: "" });
            from = gt + 1;
            continue;
        }
        from = start + 1;
    }
    out
}

/// `/\b<name>\s*=\s*"([^"]+)"/.exec(attributes)?.[1]`.
pub fn attribute<'a>(attributes: &'a str, name: &str) -> Option<&'a str> {
    let bytes = attributes.as_bytes();
    let mut from = 0;
    while let Some(found) = attributes[from..].find(name) {
        let start = from + found;
        from = start + 1;
        if start > 0 && is_word_byte(bytes[start - 1]) {
            continue;
        }
        let rest = attributes[start + name.len()..].trim_start_matches(crate::text::is_js_whitespace);
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start_matches(crate::text::is_js_whitespace);
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        match rest.find('"') {
            Some(end) if end > 0 => return Some(&rest[..end]),
            _ => continue,
        }
    }
    None
}

/// `/<tag>([^<]+)<\/tag>/.exec(body)?.[1]`.
pub fn element<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut from = 0;
    while let Some(found) = body[from..].find(&open) {
        let start = from + found + open.len();
        from = from + found + 1;
        let end = body[start..].find('<').map_or(body.len(), |offset| start + offset);
        if end > start && body[end..].starts_with(&close) {
            return Some(&body[start..end]);
        }
    }
    None
}

/// `Date.parse` for route times. Text without a UTC offset is refused rather
/// than read in the computer's own timezone.
pub fn route_time_ms(text: &str) -> Option<i64> {
    instant::parse_instant(text)
}

/// A track point for summaries.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPoint {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_m: Option<f64>,
    pub at: String,
    pub at_ms: i64,
}

/// `parseGpx`: points with a finite position and a readable time.
pub fn parse_gpx(text: &str) -> Vec<TrackPoint> {
    let mut points = Vec::new();
    for found in track_points(text) {
        let latitude = attribute(found.attributes, "lat").map_or(f64::NAN, js_number);
        let longitude = attribute(found.attributes, "lon").map_or(f64::NAN, js_number);
        if !latitude.is_finite() || !longitude.is_finite() {
            continue;
        }
        let Some(at) = element(found.body, "time").map(js_trim).filter(|at| !at.is_empty()) else {
            continue;
        };
        let Some(at_ms) = route_time_ms(at) else {
            continue;
        };
        let elevation = element(found.body, "ele").map_or(f64::NAN, js_number);
        points.push(TrackPoint {
            latitude,
            longitude,
            elevation_m: elevation.is_finite().then_some(elevation),
            at: at.to_string(),
            at_ms,
        });
    }
    points
}

/// One kilometre split, or the partial remainder.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteSplit {
    pub label: String,
    pub duration_s: f64,
    pub distance_m: f64,
}

/// What a route file says about the workout it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteSummary {
    pub start_ms: i64,
    pub end_ms: i64,
    pub splits: Vec<RouteSplit>,
    pub distance_m: f64,
    pub elevation_gain_m: f64,
}

/// `Math.round`.
fn js_round(value: f64) -> f64 {
    crate::model::grade::js_round(value)
}

/// `summarizeRoute`: splits with times interpolated at each kilometre, and
/// climbing above the noise threshold. Nothing for fewer than two points.
pub fn summarize_route(mut points: Vec<TrackPoint>) -> Option<RouteSummary> {
    if points.len() < 2 {
        return None;
    }
    // A stable sort, as Array.prototype.sort is.
    points.sort_by_key(|point| point.at_ms);
    let start_ms = points[0].at_ms;
    let mut cumulative_m = 0.0;
    let mut elevation_gain_m = 0.0;
    let mut split_start_ms = start_ms as f64;
    let mut next_boundary_m = SPLIT_DISTANCE_M;
    let mut last_elevation = points[0].elevation_m;
    let mut splits = Vec::new();
    for pair in points.windows(2) {
        let (previous, current) = (&pair[0], &pair[1]);
        let segment_m = haversine_m(previous.latitude, previous.longitude, current.latitude, current.longitude);
        let previous_cumulative_m = cumulative_m;
        cumulative_m += segment_m;
        match (current.elevation_m, last_elevation) {
            (Some(elevation), Some(last)) => {
                let rise = elevation - last;
                if rise.abs() >= ELEVATION_NOISE_M {
                    if rise > 0.0 {
                        elevation_gain_m += rise;
                    }
                    last_elevation = Some(elevation);
                }
            }
            (Some(elevation), None) => last_elevation = Some(elevation),
            _ => {}
        }
        // One leg can cross more than one boundary when GPS dropped out.
        while cumulative_m >= next_boundary_m {
            let previous_ms = previous.at_ms as f64;
            let current_ms = current.at_ms as f64;
            let fraction = if segment_m > 0.0 { (next_boundary_m - previous_cumulative_m) / segment_m } else { 1.0 };
            // Math.min(1, Math.max(0, fraction)); NaN stays NaN.
            let boundary_ms = previous_ms + (current_ms - previous_ms) * fraction.clamp(0.0, 1.0);
            splits.push(RouteSplit {
                label: format!("km {}", splits.len() + 1),
                duration_s: js_round((boundary_ms - split_start_ms) / 100.0) / 10.0,
                distance_m: SPLIT_DISTANCE_M,
            });
            split_start_ms = boundary_ms;
            next_boundary_m += SPLIT_DISTANCE_M;
        }
    }
    let end_ms = points[points.len() - 1].at_ms;
    let remainder_m = cumulative_m - (next_boundary_m - SPLIT_DISTANCE_M);
    if remainder_m >= 100.0 {
        splits.push(RouteSplit {
            label: format!("km {} (partial)", splits.len() + 1),
            duration_s: js_round((end_ms as f64 - split_start_ms) / 100.0) / 10.0,
            distance_m: js_round(remainder_m),
        });
    }
    Some(RouteSummary {
        start_ms,
        end_ms,
        splits,
        distance_m: js_round(cumulative_m),
        elevation_gain_m: js_round(elevation_gain_m),
    })
}

/// `routeFromGpx`: the points a named route file stores, and an example for
/// each point it cannot store.
pub fn route_from_gpx(text: &str) -> (Vec<RoutePoint>, Vec<String>) {
    let mut points = Vec::new();
    let mut skipped = Vec::new();
    for (index, found) in track_points(text).into_iter().enumerate() {
        let index = index + 1;
        let latitude_text = attribute(found.attributes, "lat");
        let longitude_text = attribute(found.attributes, "lon");
        let latitude = latitude_text.map_or(f64::NAN, js_number);
        let longitude = longitude_text.map_or(f64::NAN, js_number);
        let Some((recorded_at, receipt)) = element(found.body, "time").map(js_trim).and_then(route_instant) else {
            skipped.push(format!("track point {index} had no usable time"));
            continue;
        };
        if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
            skipped.push(format!("track point {index} latitude \"{}\"", latitude_text.unwrap_or("(none)")));
            continue;
        }
        if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
            skipped.push(format!("track point {index} longitude \"{}\"", longitude_text.unwrap_or("(none)")));
            continue;
        }
        let elevation = element(found.body, "ele").map(js_trim).filter(|elevation| !elevation.is_empty());
        points.push(RoutePoint {
            recorded_at,
            latitude_deg: latitude,
            longitude_deg: longitude,
            input_timestamp: Some(receipt),
            supplied_fields: elevation.map(|elevation| {
                let mut fields = crate::json::Map::new();
                fields.insert("ele".into(), crate::json::Value::String(elevation.to_string()));
                fields
            }),
        });
    }
    (points, skipped)
}

/// `(Z|[+-]\d{2}:?\d{2})$` with the `i` flag: the offset as written.
pub fn trailing_offset_loose(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.last().is_some_and(|byte| byte.eq_ignore_ascii_case(&b'z')) {
        return Some(&text[text.len() - 1..]);
    }
    let digits = |slice: &[u8]| slice.iter().all(u8::is_ascii_digit);
    for width in [6, 5] {
        if bytes.len() >= width {
            let tail = &bytes[bytes.len() - width..];
            let signed = matches!(tail[0], b'+' | b'-');
            let shaped = if width == 6 {
                digits(&tail[1..3]) && tail[3] == b':' && digits(&tail[4..6])
            } else {
                digits(&tail[1..5])
            };
            if signed && shaped {
                return Some(&text[text.len() - width..]);
            }
        }
    }
    None
}

/// `routeInstant`: a stored instant and receipt for route time text that
/// carries its own UTC offset.
fn route_instant(text: &str) -> Option<(String, InputTimestampReceipt)> {
    let offset = trailing_offset_loose(text)?;
    let millis = route_time_ms(text)?;
    let recorded_at = crate::model::time::format_utc_millis(millis);
    Some((
        recorded_at,
        InputTimestampReceipt {
            timestamp_text: Some(text.to_string()),
            utc_offset_text: Some(offset.to_string()),
            utc_offset_meaning: Some(UtcOffsetMeaning::MeaningNotDocumented),
            source_time_zone: None,
            source_time_zone_meaning: None,
            timestamp_basis: Some(InputTimestampBasis::SourceOffsetTimestamp),
            timestamp_missing_reason: None,
            utc_offset_missing_reason: None,
        },
    ))
}

/// True when route points and the workout's window overlap.
pub fn overlaps(start_ms: Option<i64>, end_ms: Option<i64>, points: &[RoutePoint]) -> bool {
    let (Some(start_ms), Some(end_ms)) = (start_ms, end_ms) else {
        return false;
    };
    let mut times = points.iter().filter_map(|point| instant::parse_instant(&point.recorded_at));
    let Some(first) = times.next() else {
        return false;
    };
    let (low, high) = times.fold((first, first), |(low, high), time| (low.min(time), high.max(time)));
    low < end_ms && high > start_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpx(points: &str) -> String {
        format!("<gpx><trk><trkseg>{points}</trkseg></trk></gpx>")
    }

    #[test]
    fn a_degree_of_latitude_is_about_111_km_and_a_point_is_zero() {
        assert!((haversine_m(0.0, 0.0, 1.0, 0.0) - 111_195.0).abs() < 50.0);
        assert_eq!(haversine_m(37.7749, -122.4194, 37.7749, -122.4194), 0.0);
    }

    #[test]
    fn reads_the_track_point_shapes_the_expression_read() {
        let text = r#"<trkpt lat="1" lon="2"/><trkpt lat="3" lon="4"><time>T</time></trkpt><trkpts lat="9"/>"#;
        let found = track_points(text);
        // The self-closing point runs on to the next closing tag, as the
        // expression's first alternative matched it.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attributes, r#" lat="1" lon="2"/"#);
        assert!(found[0].body.contains("<time>T</time>"));
        let alone = track_points(r#"<trkpt lat="1" lon="2"/>"#);
        assert_eq!(alone, vec![TrackMatch { attributes: r#" lat="1" lon="2""#, body: "" }]);
        assert!(track_points("<trkpt lat=\"1\">").is_empty());
    }

    #[test]
    fn attributes_need_a_word_boundary_and_a_value() {
        assert_eq!(attribute(r#" data-lat="5" lat="6""#, "lat"), Some("5"));
        assert_eq!(attribute(r#" xlat="5" lat = "6""#, "lat"), Some("6"));
        assert_eq!(attribute(r#" latitude="5""#, "lat"), None);
        assert_eq!(attribute(r#" lat="" lat="7""#, "lat"), Some("7"));
    }

    #[test]
    fn elements_need_text_before_the_next_tag() {
        assert_eq!(element("<time></time><time>x</time>", "time"), Some("x"));
        assert_eq!(element("<ele>1<b/></ele>", "ele"), None);
        assert_eq!(element("<ele> 12 </ele>", "ele"), Some(" 12 "));
    }

    #[test]
    fn a_point_needs_a_position_and_a_time_with_an_offset() {
        let text = gpx(concat!(
            r#"<trkpt lat="37.0" lon="-122.0"><ele>10</ele><time>2026-08-09T19:00:00Z</time></trkpt>"#,
            r#"<trkpt lat="37.1" lon="-122.0"><ele>11</ele></trkpt>"#,
            r#"<trkpt lat="37.2" lon="-122.0"><time>2026-08-09T19:00:05</time></trkpt>"#,
            r#"<trkpt lat="x" lon="-122.0"><time>2026-08-09T19:00:06Z</time></trkpt>"#,
            r#"<trkpt lat="37.3" lon="-122.0"><ele>  </ele><time>2026-08-09T19:00:07Z</time></trkpt>"#,
        ));
        let points = parse_gpx(&text);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].elevation_m, Some(10.0));
        // `Number("  ")` is 0 in JavaScript.
        assert_eq!(points[1].elevation_m, Some(0.0));
        assert!(parse_gpx("<gpx></gpx>").is_empty());
    }

    #[test]
    fn a_named_route_reports_each_point_it_cannot_store() {
        let text = gpx(concat!(
            r#"<trkpt lat="12.5" lon="56.5"><ele> 200 </ele><time>2026-06-14T08:01:00-05:00</time></trkpt>"#,
            r#"<trkpt lat="120" lon="56.5"><time>2026-06-14T08:02:00-05:00</time></trkpt>"#,
            r#"<trkpt lat="12.5" lon="560"><time>2026-06-14T08:03:00-0500</time></trkpt>"#,
            r#"<trkpt lat="12.5" lon="56.5"><time>2026-06-14T08:04:00</time></trkpt>"#,
            r#"<trkpt lon="56.5"><time>2026-06-14T08:05:00z</time></trkpt>"#,
        ));
        let (points, skipped) = route_from_gpx(&text);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].recorded_at, "2026-06-14T13:01:00.000Z");
        assert_eq!(points[0].supplied_fields.as_ref().unwrap()["ele"], "200");
        assert_eq!(
            skipped,
            vec![
                "track point 2 latitude \"120\"",
                "track point 3 longitude \"560\"",
                "track point 4 had no usable time",
                "track point 5 latitude \"(none)\"",
            ]
        );
    }

    fn track(paces: &[f64]) -> String {
        // A straight northward track at given paces, points every five seconds.
        let start = instant::parse_instant("2026-08-09T19:00:00-07:00").unwrap();
        let mut latitude = 37.7749;
        let mut at = start;
        let (mut km, mut into) = (0, 0.0);
        let mut out = String::new();
        let mut step = 0;
        while step < 2000 && km < paces.len() {
            let distance = 1000.0 / paces[km] * 5.0;
            latitude += distance / 111_320.0;
            into += distance;
            at += 5000;
            out.push_str(&format!(
                r#"<trkpt lat="{latitude:.6}" lon="-122.4194"><ele>20.0</ele><time>{}</time></trkpt>"#,
                crate::model::time::format_utc_millis(at)
            ));
            if into >= 1000.0 {
                km += 1;
                into -= 1000.0;
            }
            step += 1;
        }
        gpx(&out)
    }

    #[test]
    fn splits_interpolate_each_kilometre_and_show_a_fade() {
        let even = summarize_route(parse_gpx(&track(&[300.0, 300.0]))).unwrap();
        assert!((even.splits[0].duration_s - 300.0).abs() < 0.5);
        assert!((even.splits[1].duration_s - 300.0).abs() < 0.5);
        assert_eq!(even.splits[0].distance_m, 1000.0);
        let fade = summarize_route(parse_gpx(&track(&[300.0, 300.0, 420.0]))).unwrap();
        assert_eq!(fade.splits.len(), 3);
        assert!(fade.splits[2].duration_s > 400.0);
        let mut reversed = parse_gpx(&track(&[300.0, 300.0]));
        reversed.reverse();
        let summary = summarize_route(reversed).unwrap();
        assert!(summary.start_ms < summary.end_ms);
        assert!(summary.splits[0].duration_s > 0.0);
    }

    #[test]
    fn climbing_ignores_wobble_and_counts_real_rises() {
        let start = instant::parse_instant("2026-08-09T19:00:00Z").unwrap();
        let points = |elevation: &dyn Fn(usize) -> f64, count: usize| {
            gpx(&(0..count)
                .map(|index| {
                    format!(
                        r#"<trkpt lat="{:.6}" lon="-122.4194"><ele>{}</ele><time>{}</time></trkpt>"#,
                        37.7749 + index as f64 * 0.00009,
                        elevation(index),
                        crate::model::time::format_utc_millis(start + index as i64 * 5000)
                    )
                })
                .collect::<String>())
        };
        let jitter = points(&|index| 20.0 + if index % 2 == 0 { 0.4 } else { -0.4 }, 200);
        assert_eq!(summarize_route(parse_gpx(&jitter)).unwrap().elevation_gain_m, 0.0);
        let climb = points(&|index| 20.0 + index as f64 * 2.0, 100);
        assert_eq!(summarize_route(parse_gpx(&climb)).unwrap().elevation_gain_m, 198.0);
    }

    #[test]
    fn too_short_a_track_has_no_summary() {
        assert!(summarize_route(Vec::new()).is_none());
        let one =
            parse_gpx(&gpx(r#"<trkpt lat="37" lon="-122"><ele>10</ele><time>2026-08-09T19:00:00Z</time></trkpt>"#));
        assert!(summarize_route(one).is_none());
    }
}
