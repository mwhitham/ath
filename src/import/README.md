# Provider imports

A provider route turns one kind of export into records. The route supplies a
reader. The import operation in `provider.rs` does everything else: staging,
source resolution, matching, validation, the preview, and the write. A new
route needs no SQL and no new command code.

## The operation

`import::preview(store, path, options)` detects an Apple Health, WHOOP, or
Oura export, runs its reader, and returns a `ProviderPreview`.
`preview.apply(&mut store)` writes it. The other routes enter the same
operation through `import::preview_shared` (Garmin and Strava),
`import::preview_mapped` (a JSON file read through a mapping file), and
`import::preview_pull` (one day `runtime::garmin_pull::fetch_day` read).

1. The reader sends candidates to a `CandidateSink`. The sink validates each
   one as a complete current record and stages it in a private temporary
   SQLite file. Nothing is held in memory per record.
2. Matching opens one read transaction on the live database. It resolves
   each declared source, finds each candidate's stored record by its match
   key, and decides `new`, `matching`, `changed`, or a conflict.
3. The preview lists those decisions and the database revision they were
   read at. It holds no lock.
4. `apply` opens one write transaction. It refuses the plan when the database
   revision moved, a corrected record's revision moved, or a key now names
   another record. It writes in bounded batches and advances the database
   revision once. A plan with nothing to write changes nothing.

## Writing a reader

Implement `ProviderReader` and register it in `provider::reader_for`.
Recognition of the export belongs in `detect.rs`.

```rust
pub trait ProviderReader {
    fn route(&self, export: &DetectedExport) -> Route { Route::for_export(export) }
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()>;
}
```

A reader that holds its own input, such as a path chosen by `classify` or
responses already fetched, implements `RouteReader` instead and returns its
`Route`. `import::preview_route(store, reader, options)` runs it.

```rust
pub trait RouteReader {
    fn route(&self) -> Route;
    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()>;
}
```

A reader may read files with the helpers in `input.rs`, `detect.rs`
(`read_csv_bundle`), `csv.rs`, `units.rs`, and `shared.rs` (`SharedFiles`). It
must not open the athlete database, decide whether a record is new, or prompt.

A reader sends candidates in plan order within each section. When a record
cannot be sent as it is read, the reader keeps it in its own private
temporary file, never in a list that grows with the export. The Apple Health
reader does this: it streams `export.xml`, keeps samples, sleep fragments,
and workouts in `apple/scratch.rs` until their order and route files are
known, and then sends them.

The sink offers these calls:

- `export_source(writer, vendor, sensor) -> String` declares a source and
  returns the ID to put in `source_id`. One writer, route, and sensor is one
  source. The source's detail follows the 2.1.0 wording.
- `manual_source() -> String` returns the shared source for hand-entered values.
- `received_at() -> &str` is the import time for each measured record's
  `received_at`, with `received_at_basis` set to `import_time`.
- `add(candidate)` validates and stages one record.
- `skip(reason, example)` and `skip_count(reason, count)` count an omitted row
  or object. The preview lists reasons in the 2.1.0 order with the first example.

A keyed route (below) reports differently, as its 2.1.0 command did:

- `route_source(writer, vendor, sensor) -> String` declares a source with no
  detail.
- `leave_out(line)` reports an omitted object, in input order.
- `begin_group(label)` starts the records of one file, activity, or row. When
  matching leaves one of them out because its stored record or its source is
  deleted, the line starts with `label` and takes the group's place.
- `understood(field)` and `preserved(field)` note the input fields stored as
  typed values and the fields kept under their own names.
  `understood_under(source, field)` and `preserved_under(source, field)` count
  a field only when that source is not deleted.

## What a candidate contains

A `Candidate` has three parts:

- `data`: the complete `RecordData`, including every receipt the record
  stores: input timestamp text and offset, the value and unit the source wrote,
  provider reporting periods, and `field_basis`. Use the helpers in
  `candidate.rs` (`source_value`, `source_reported_summaries`,
  `offset_timestamp_receipt`, `missing_timestamp_receipt`, `value_basis`) so
  every route records them the same way. A measured record or device must name
  a source from `export_source`, `route_source`, or `manual_source`. An athlete
  report or nutrition entry names none.
- `key`: the `MatchKey` that finds the record again on a later import.
- `section`: where the record sits in the plan. Sources come first, then
  `Device`, `Record`, `Summary`, `Sample`, and `Report`. Within a section,
  records keep the order the reader sent them.

Turn provider text into a stored instant with `instant::normalized_utc`, which
reads offset text as 2.1.0 did. Never read text without an offset in the
computer's own timezone.

## Match keys and collision groups

The stored key is the JSON text of the key parts. `KeyPart::Source` stands for
the candidate's resolved source ID. Because the source is part of the key, two
devices that describe one workout remain two records.

`MatchKey::numbered(parts)` appends the candidate's position among candidates
with the same parts: 0, then 1. Those candidates form a collision group. On a
later import, identical rows match their stored records again. When a group of
two or more contains a changed record, the preview reports a conflict and the
write is refused, because the importer cannot tell which row was corrected.

`MatchKey::unique(parts)` has no number and no group. Use it for something an
export names once, such as a device.

The helpers `MatchKey::measured`, `MatchKey::session`, and
`MatchKey::athlete_report` build the keys the 2.1.0 provider routes used. Keep
an existing route's key shape exactly. A changed key makes every stored record
look new.

A stored record matches a candidate when their data agree after `received_at`
and `received_at_basis` are removed at every depth. Any other difference makes
the candidate a correction, stored as a new revision of the same record. A key
that names a deleted record or another record type is a conflict.

## Keyed routes

A route's `policy` says how its candidates find their stored records.
Apple Health, WHOOP, and Oura use `MatchPolicy::Provider`, described above.
Garmin and Strava files, the Garmin pull, and mapped imports use
`MatchPolicy::Keyed`, the rules of the 2.1.0 helper's store:

- `MatchKey::literal(text)` is stored as that text: an activity ID, a file
  name, or the escaped parts of a mapped key (D181). It has no number and no
  collision group.
- A source is found by writer, route, and sensor, deleted or not. A deleted
  source leaves out every candidate under it, and no second source is made
  (D190). A new source has no import key.
- A key that names a deleted record leaves the candidate out (D182). A key
  that names another record type refuses the import.
- A candidate matches when it equals the stored record once the stored
  `received_at` is copied onto it. `KeyedPolicy` says whether a changed
  record replaces the stored object or is laid over it, whether it keeps its
  first `received_at`, and which basis its revision names.
- A later candidate with an earlier candidate's key repeats it when the data
  agree; its record is listed again in `record_ids`. Different data refuse
  the import.

`route.report` chooses the result `report_value` builds: the provider preview,
or the `source`, `mapping`, `record_ids`, `outcome`, `understood`,
`preserved`, and `left_out` fields 2.1.0 printed for the keyed routes.

## Proof

Each route compares its output with the 2.1.0 implementation.
`tools/reference/import-golden.ts` (WHOOP), `import-golden-apple.ts`
(Apple Health), and `route-golden.ts` (Oura, Garmin, Strava, the Garmin pull,
and mapped imports) run the old program on synthetic inputs and write golden
files under `tests/golden/`. A Rust test replays the same steps and
requires the same preview, write result, stored JSON for every revision, and
import keys. The keyed routes ran in the 2.1.0 helper, which read the clock and
wrote its own JSON, so their comparison names each normalization in
`tests/route_support/keyed.rs`. Name golden files without provider words; the
public-data check reads such names as health exports.
