# Size of a vendor original in a package

This study was measured on 16 November 2026 for D242. It used no real
data. Every value below is invented.

## Method

`ath export` at `0de71bb` writes each record and each revision with
`json::to_deterministic_pretty` in `src/json.rs`. That printer puts two
spaces before each level, sorts keys, and writes `": "` after each key.
`ArrayWriter::item` in `src/package/export.rs` then indents every line
four more spaces and writes `,` and a new line before each item. The
stored `record_revisions.record_json` is compact. A short Python script
copied those rules and the JavaScript number text for these values. It
then measured one synthetic original.

The original follows the pinned Polar `ExerciseJSON` model in
`backend/app/schemas/providers/polar/exercise_import.py` at Open
Wearables commit `183e8cc4f6de897dbd288c93d49a691f2cf721ae`. It is a
one-hour run:

- a route of 3,600 points, one a second. Each point has `latitude` and
  `longitude` to six decimals, `time`, `satellites`, and `fix`.
- six sample strings with one value a second: heart rate, speed,
  cadence, altitude, temperature, and distance
- five heart-rate zones, and the model's summary fields
- the envelope in section 2 of the specification. `list` has no samples,
  zones, or route. `detail` has all three.
- the `source_report` fields around the envelope, with invented ids and
  times

A second shape drops the samples and the zones and keeps the route.

## Results

| Shape | Compact `record_json` | One exported item | Ratio | Exported, one revision | Exported, two revisions |
|---|---|---|---|---|---|
| Route, samples, and zones | 412,909 bytes | 847,259 bytes | 2.05 | 1,694,355 bytes | 2,541,611 bytes |
| Route only | 311,189 bytes | 744,471 bytes | 2.39 | 1,488,779 bytes | 2,233,247 bytes |

An original with one revision is in the main document twice: once as its
current record and once as that revision. So it exports at about four
times its compact column bytes. Each later revision adds one more item.

Under 256 MiB of exported bytes:

- 158 originals of the first shape fit with one revision each, or 105
  with two.
- 180 originals of the route-only shape fit with one revision each, or
  120 with two.

The earlier bound counted 128 MiB of compact `record_json`. With one
revision for each original, that is about 525 MiB of the first shape as
export writes it, and about 612 MiB of the route-only shape. Both are
over the 512 MiB package read cap.

The deepest value is a route point's field, five levels inside the
record's `data`. In the package it is eight levels deep. A package reads
back at 127 levels or fewer. `check_nesting` allows 128, but serde_json
1.0.154 refuses 128 when the read parses the document.

## What was not measured

- A real vendor response. Route point text and sample strings differ by
  device and by sport.
- Garmin, Strava, and Suunto details. Their laps, splits, and sample
  arrays nest in other ways.
- How long measuring the stored originals takes in a database near the
  bound.
