# Strava bulk account export importer plan (proposed)

**Status:** proposed draft for review. **Do not merge this pull request.** After
approval, append the accepted decisions to `build-history/v1.0.0/decisions.md` and
update `build-history/v1.0.0/progress.md`. This file stays in this appendix as the
long-form plan linked from those D-numbers.

**Goal:** Let an athlete load their own Strava **account download** (bulk ZIP from
Strava settings) into `athlete.ath.sqlite` through the open `ath` CLI, without
Strava OAuth, without the Strava API, and without a hosted sync service. The import
path is local files only. First implementation release targets **Node (CLI)** and
**iOS (via `ath-uniffi`)** only, matching the Rust adoption direction on draft PR
[#20](https://github.com/mwhitham/ath/pull/20).

**Why agent-neutral access:** Strava’s official MCP integration is tied to one vendor
stack. A bulk-export importer in the public CLI lets any agent read the same ATH files
the project already defines.

### Verified versions (checked 2026-10-23)

| Field | Value | Where checked |
| --- | --- | --- |
| `package.json` `version` | **1.1.0** | `/workspace/package.json` on `main` |
| `ATH_PACKAGE_VERSION` | **1.1.0** | `src/version.ts` on `main` |
| npm registry `latest` | **1.0.0** | `npm view athleticstandard version` |
| Record format `athleticstandard_version` | **1.0.0** | `SPEC.md`, `src/v1/schema.ts` |
| Latest decision on `main` | **D129** | `build-history/v1.0.0/decisions.md` |

Draft PR [#33](https://github.com/mwhitham/ath/pull/33) (`cursor/release-prep-951e`)
adds D130–D151 on its branch for npm 1.1.0 release prep. Draft PR #20 reserves
D130–D132 for Rust adoption. Draft PR #31 proposes D133–D144 for Garmin in
`plans/garmin-support.md` only. **This plan proposes Strava open decisions starting at
D152** so none of those ranges collide. D147 on PR #33 notes draft plans may renumber
later; the build PR should re-check `decisions.md` on `main` before appending.

---

## 1. Plan location (Rule 0)

`build-history/README.md` requires executable plans in a versioned folder under
`build-history/`, with long-form material in `appendix/` and decisions in
`decisions.md`. `AGENTS.md` repeats that rule. This plan does **not** change the public
record format version, so it lives in **`build-history/v1.0.0/appendix/`** (same pattern
as `contribution-readiness.md` and draft `rust-adoption.md` on PR #20). It does **not**
use a top-level `plans/` directory (draft PR #31 used that path; README does not).

---

## 2. What Strava ships in a bulk account export

### 2.1 How the athlete obtains the archive (verified)

Strava documents bulk export on the help center:

1. Log in on strava.com.
2. Settings → **My Account** → **Download your account** → **Get Started** →
   **Request download**.
3. Strava emails a link (often hours later).

Sources:

- [How Do I Export My Strava Data?](https://support.strava.com/en-us/articles/15401919-how-do-i-export-my-strava-data) (includes bulk export steps and per-activity FIT/GPX/TCX export menus).
- [Exporting Your Data and Bulk Export](https://support.strava.com/en-us/articles/15401919-exporting-your-data-and-bulk-export) (same article family on the help center).

The privacy policy describes **access and portability** and points athletes to those
download instructions:
[Strava Privacy Policy — Your Privacy Rights](https://www.strava.com/legal/privacy).

### 2.2 Archive layout (partially verified)

Strava does **not** publish a field-level ZIP specification in developer docs. Layout
below is from the help article (high level) and from independent write-ups that list
extracted folder contents. Treat anything not in Strava’s own docs as **unverified**
until confirmed with synthetic fixture design in the build PR.

**Verified (Strava help):**

- Bulk export is a ZIP archive of account data requested from settings.
- Individual activities may appear as original **FIT**, or as **GPX** / **TCX** exports
  from activity pages (separate from bulk layout).

**Observed in third-party documentation (unverified against a live 2026 export):**

A typical extracted root contains at least:

| Path | Role |
| --- | --- |
| `activities.csv` | Index of activities with metadata columns |
| `activities/` | One file per activity (see §2.3) |
| `routes/` + `routes.csv` | Saved routes (often GPX) |
| `profile.csv`, `profile.jpg` | Profile metadata and photo |
| `bikes.csv`, `shoes.csv` | Equipment catalogs |
| Many other `*.csv`, `*.json`, PDFs, `clubs/`, `media/` | Social, commerce, preferences, legal PDFs |

Example tree:
[Moving from Strava (geo-activity-playground)](https://martin-ueding.github.io/geo-activity-playground/moving-from-strava.html)
(mirror in that project’s repo docs).

### 2.3 Activity files in `activities/` (unverified details, widely reported)

Community tools and importers report these extensions under `activities/`:

- `.fit`, `.fit.gz`
- `.gpx`, `.gpx.gz`
- `.tcx`, `.tcx.gz`

The `activities.csv` **`Filename`** column holds the path to the activity file (for
example `activities/1234567890.fit.gz`). **`Filename` is the join key**, not the numeric
token in the basename alone and not always equal to **`Activity ID`** in the same row.
See [Strava community thread on ID mismatch](https://communityhub.strava.com/developers-api-7/strava-bulk-data-activity-file-ids-don-t-match-the-activities-csv-activity-id-1805).

Empty `Filename` means Strava did not include an original file (manual entry, deleted
upload, or activity type without a file). **Unverified:** frequency and exact rules.

### 2.4 `activities.csv` columns (unverified complete list)

Strava does not publish a stable CSV schema. A community SQL external-table definition
lists many column names observed in real exports (treat as planning hints, not a contract):
[davidkurtz/strava `1b_create_activities_ext.sql`](https://github.com/davidkurtz/strava/blob/master/1b_create_activities_ext.sql).

Columns the importer build should handle explicitly (names may drift; parser must fail
loudly on required missing columns):

| Column (reported) | Planned use in ATH |
| --- | --- |
| `Activity ID` | `provider_record_id` on workout and import keys |
| `Activity Date` | Start instant (see §7) |
| `Activity Name` | Workout label / `activity` string |
| `Activity Type` | Map to ATH activity label |
| `Filename` | Link to `activities/` file |
| `Elapsed Time`, `Moving Time` | Session bounds and aggregates (seconds) |
| `Distance` / `Distance_m` | Distance aggregate (confirm unit in fixture) |
| `Elevation Gain`, `Elevation Loss` | Aggregates |
| `Average Speed`, `Max Speed` | Aggregates (confirm km/h vs m/s in fixture) |
| `Average Heart Rate`, `Max Heart Rate` | Aggregates; series from file when present |
| `Average Watts`, `Max Watts`, `Weighted Average Power` | Cycling power aggregates |
| `Calories` | Optional aggregate only (not a vendor composite score) |
| `Commute` | Metadata on workout |
| `Activity Gear`, `Bike`, `Gear` | Equipment strings (extensions or notes) |

All other columns: preserve in import receipt / skip list unless a later decision
expands scope.

---

## 3. Import scope

### 3.1 In scope (v1 Strava bulk route)

| Export part | ATH mapping | Why |
| --- | --- | --- |
| `activities.csv` rows with parseable times | `workout_session` measured records | Core training history |
| Linked files in `activities/` (FIT/GPX/TCX, gzip) | Time series (`measurement_series`), route geometry, laps/segments where the parser exposes them | Dense samples and outdoor routes belong in ATH, not only CSV totals |
| Strava as `source` / `source_device` | `source` records with `data_route` `strava_bulk` | Two-tier wall: measured rows need a source (D85, D108) |
| Import preview + confirmed apply | `record_import_keys` with `strava_bulk/v1` | D91, D97, D108 |

### 3.2 Out of scope (v1)

| Export part | Reason |
| --- | --- |
| Social graph (`followers.csv`, `following.csv`, `clubs/`, kudos, comments, posts) | Not training or recovery measurements; not athlete-reported logs |
| `media/`, `profile.jpg` | Photos are not v1 workout/recovery series |
| `segments.csv`, leaderboards, `local_legend_segments.csv` | Segment competition data; no v1 record type |
| `orders.csv`, `support_tickets.csv`, legal PDFs | Non-athletic data |
| `connected_apps.csv`, `applications.csv` | Account metadata |
| `routes/` saved routes | No v1 “planned route” record; optional later via attachments |
| Strava **API** or OAuth pull | Different trust, rate, and legal model; out of this plan |
| Multi-athlete archives | Single-athlete product boundary |
| Guessing missing activity files | Skip with counted reason; never invent samples |

### 3.3 Explicit non-goals

- No Strava MCP server in this repo (D7, D121).
- No hosted “ath connect” broker.
- No merging Strava **Relative Effort** or similar vendor composites into ATH
  `provider_score` unless a separate decision defines evidence (same rule as Garmin
  plan §4 and D110).

---

## 4. Mapping to existing ATH code

### 4.1 CLI and import pipeline today

| Step | Module | Behavior today |
| --- | --- | --- |
| Command | `src/program.ts` | `ath import <path>` → provider or package path |
| Help | `src/help.ts`, `skill/references/cli.md` (generated) | Lists Apple, WHOOP, Oura only |
| Detection | `src/import/detect.ts` | ZIP, directory, or file markers for three vendors |
| Parse | `src/import/index.ts` | `parseProviderExport` → vendor parser → `ImportPayload` |
| Legacy merge | `src/import/merge.ts` | 0.x-shaped payload; still used inside parsers |
| v1 apply | `src/db/provider-import.ts` | Maps payload → v1 records, `record_import_keys`, preview/apply |
| GPX routes | `src/import/routes.ts`, `src/import/apple.ts` | Apple `workout-routes/*.gpx` only |
| ZIP limits | `src/import/zip.ts` | Entry size caps, symlink refusal (D94) |

**Planned additions (build PR only, not this draft):**

- `src/import/strava.ts` — CSV + activity-file orchestration.
- `src/import/detect.ts` — Strava markers: root `activities.csv` plus `activities/`
  directory inside ZIP or folder.
- `src/import/index.ts` — `format: "strava"`, route `strava_bulk`, label
  `Strava account export`.
- Optional shared parsers under `ath-core` (Rust) per PR #20 — see §6.

### 4.2 v1 record types

Primary target: **`workout_session`** (`WorkoutSessionData` in `src/v1/schema.ts`).

Secondary (when activity files contain samples):

- **`measurement_series`** for HR, power, cadence, GPS-derived quantities, etc.
- **`point_measurement`** only when the route stores a single summary point not
  tied to a series (prefer series when samples exist).

**`provider_summary`:** use for CSV-only totals that are clearly source-reported
(duration, distance, elevation) when no file provides better evidence. Do not use for
Strava proprietary effort scores without D110-style approval.

**Attachments:** optional v1 extension: store raw activity file hash under
`attachments/` for audit, similar to package attachment rules in `SPEC.md` §9. Open
decision D157.

### 4.3 Provider import keys (D108)

New method: **`strava_bulk/v1`**.

| Record kind | Match key ingredients |
| --- | --- |
| `source` | `data_writer`, `data_route`, `sensor_type` (same pattern as other routes in `provider-import.ts`) |
| `workout_session` | `source_id`, start instant (normalized UTC), Strava `Activity ID` as `provider_record_id` |
| `measurement_series` | `source_id`, quantity, series start instant |
| Collision ordinal | Append when CSV order produces indistinguishable rows (D112) |

Re-import: same key + same content → **matching**; same key + changed content →
**revision**; ambiguous duplicate change → **conflict**, no guess (D91, D108, D112).

### 4.4 Overlap with Apple Health, WHOOP, Oura

Those routes populate sleep, HRV, and daily strain. Strava bulk export is
**workout-centric**. Sleep and nightly HRV from Strava are **not** in scope unless a
future export study shows a reliable file (none in §2.2).

### 4.5 Overlap with draft Garmin plan (PR #31)

| Topic | Garmin plan | Strava plan |
| --- | --- | --- |
| FIT decoding | Shared `ath-core` FIT module | **Reuse the same decoder** for `activities/*.fit(.gz)` |
| GPX | Activity and route GPX | Reuse GPX track parsing; generalize beyond Apple-only `routes.ts` callers |
| TCX | Mentioned for Garmin contexts | Strava archive includes TCX; same shared parser milestone |
| Account ZIP | Garmin `DI-Connect-*` trees | Strava flat `activities.csv` + `activities/` |
| API pull | Garmin Personal API | **Not** in Strava v1 |
| Dedup across vendors | Same workout from Garmin + Apple → conflict policy D141 | Same: Strava + Garmin/Apple overlap on start time + activity → conflict, not silent merge |

**Build order:** land shared FIT/GPX/TCX parsers once (Garmin + Strava share). If
Garmin implementation merges first, Strava imports call the same library. If Strava
merges first, extract parsers before Garmin account export work. Do not ship two FIT
stacks.

---

## 5. Detection and CLI surface

### 5.1 Detection rules

Accept:

- A **directory** containing `activities.csv` and an `activities/` subdirectory.
- A **ZIP** whose entries include `activities.csv` (case-insensitive path) and at least
  one `activities/*` entry.

Reject:

- Symbolic links inside the archive (existing `detect.ts` policy).
- More than `MAX_PROVIDER_FILES` (100_000) entries.
- Text entries over `MAX_PROVIDER_TEXT_BYTES` (from `zip.ts`).

Do **not** detect from a lone `activities.csv` without the folder unless a later
decision allows CSV-only import (open question D158).

### 5.2 CLI

No new top-level command. Extend auto-detection:

```text
ath import ~/Downloads/strava_export.zip
ath import ~/Downloads/strava_export/
```

After build: run `pnpm generate:skill-cli` so `skill/references/cli.md` lists Strava
(D80).

### 5.3 Rust / Node split (PR #20 direction)

Parsing and normalization should live in **`ath-core`** once the Rust cutover for
imports reaches that milestone. Until then, a **temporary** TypeScript parser in
`src/import/strava.ts` is allowed only with a named removal step in `progress.md`
when `ath-core` exposes `strava_bulk_import` (same rule as Garmin plan §9 temporary
bridge).

---

## 6. Activity file parsing

### 6.1 Read order

For each `activities.csv` row:

1. Resolve `Filename` relative to export root.
2. If missing or file absent: emit **CSV-only** `workout_session` when times and type
   parse; count skip reason `missing_activity_file`.
3. If present: decompress `.gz` in memory within existing ZIP size limits.
4. Dispatch by extension to FIT, GPX, or TCX parser.
5. Merge file-derived start/end, samples, and laps with CSV aggregates using rules:
   - **Instant:** prefer file timestamps when present; else parse `Activity Date`.
   - **Aggregates:** prefer file integrals when present; else CSV numbers with unit
     conversion verified in tests.
   - **Conflict** between CSV and file on the same field: file wins for samples; CSV
     wins for Strava-computed fields only when the file lacks them (document per field
     in build).

### 6.2 GPX

Reuse logic from `src/import/routes.ts` (`parseGpx`, `summarizeRoute`). Lift shared
GPX helpers to a vendor-neutral module or `ath-core` so Apple and Strava do not fork.

### 6.3 FIT and TCX

**Not implemented on `main` today.** Implementation depends on draft PR #20 (`ath-core`)
and coordination with Garmin FIT work in PR #31. This plan requires one FIT stack and
one TCX stack in Rust with Node bindings.

### 6.4 Filename quirks (unverified)

Community importers report:

- `Filename` may list `.fit.gz` while only `.fit` exists (or the reverse).
- **Build must try** explicit path, then strip or add `.gz`, then record skip — never
  read arbitrary paths outside `activities/`.

---

## 7. Time zones and units

### 7.1 Timestamps (SPEC.md §3, D105)

- Normalized instants: UTC with millisecond precision in stored fields.
- Preserve original `Activity Date` text in `input_timestamp` / receipt fields.
- **`Activity Date` format** is reported as English locale strings like
  `Dec 28, 2025, 2:34:18 PM` (unverified for all locales). Parser must use a fixed
  pattern from synthetic fixtures; refuse ambiguous rows rather than guess zone.

**Open:** whether Strava exports local wall time with implicit zone vs UTC. Resolve with
fixture headers only, not real exports in Git.

### 7.2 Units

Strava CSV mixes units by column era (`Distance` vs `Distance_m`, speeds in km/h per
community docs). Importers must:

- Store ATH standard units per `SPEC.md` / `src/v1/schema.ts`.
- Keep `input_value` + `input_unit` on measurements when conversion happens.
- Refuse unknown columns rather than silently assuming km or miles.

### 7.3 Calendar vs instant

Strava activities are instant-bounded workouts, not date-only provider rows. Do not
invent midnight (SPEC.md §3).

---

## 8. Dedup, idempotency, partial and corrupt archives

### 8.1 Re-import of the same archive

Second `ath import` on an unchanged export → preview shows **matching** only; database
revision unchanged except `imported_at` on keys if we touch them — prefer **no write**
when everything matches (same as `tests/v1-provider-import.test.ts` expectations).

### 8.2 Changed Strava row

Edited activity name or corrected distance → same `strava_bulk/v1` key → **changed** →
new revision on confirm.

### 8.3 Partial ZIP

- Truncated ZIP: fail at open with clear error (`zip.ts`).
- Missing `activities/` file for one row: skip file enrichment; import CSV row if valid.
- Corrupt FIT/GPX/TCX: skip that activity’s series; keep session if CSV valid; count
  `invalid_activity_file`.

### 8.4 Overlap with other sources

When a workout from Strava matches one from Apple Health or Garmin on start instant +
activity type, apply the same **conflict** policy as draft Garmin **D141** (keep both
unless athlete resolves). Default: **keep all sources**; predictions use per-source
baselines (D31, D109).

### 8.5 Idempotency across paths

Importing the same physical workout from Strava bulk and later from Garmin export should
produce **two** `workout_session` records unless the athlete merges via conflict
resolution — never silent dedup across `match_key_method` values.

---

## 9. Data safety

Aligned with D91, D94, D124, and `CONTRIBUTING.md`:

| Guarantee | Mechanism |
| --- | --- |
| No destructive overwrite | Preview default; apply only on confirm or `--yes` |
| Atomic apply | `withWriteTransaction` in `src/db/transaction.ts` |
| No schema mutation on import | Imports add/revise records only |
| Untrusted input | Refuse symlinks, path traversal, oversized entries |
| No real health data in Git | Synthetic fixtures only; `scripts/check-public-data.ts` allowlist |
| User data preserved | Migration path unrelated; import tests run on **copies** of temp DB files |
| Verification on copies | Extend `tests/import-safety.test.ts` patterns: import twice, hash stable records |

**Explicit conversion:** if a future change rewrites existing Strava-linked records, it
must ship as a documented migration with preview, backup, and confirm (D124) — not as
a silent importer change.

---

## 10. Test plan (synthetic fixtures only)

All fixtures under `tests/fixtures/exports/strava/` with SHA-256 entries in
`scripts/check-public-data.ts`. Build the **smallest** ZIP that still proves behavior.

| ID | Intent | Failure signal |
| --- | --- | --- |
| T1 | Minimal valid ZIP: 2-row `activities.csv`, one `.gpx`, one CSV-only row | Removing parser breaks `ath import` preview counts |
| T2 | `.gpx.gz` decompression | Wrong gzip handling → thrown error or empty series |
| T3 | `Filename` / missing file | Missing file → skip reason `missing_activity_file`, session still created |
| T4 | Second identical import | New/changed counts zero; keys stable |
| T5 | Changed activity name in CSV | `changed_records` = 1, revision increments |
| T6 | Unit conversion column | Wrong distance unit → assertion on stored meters |
| T7 | `Activity ID` collision handling | Duplicate IDs in CSV → conflict, not double write |
| T8 | Invalid GPX | Corrupt file → session from CSV only, skip counted |
| T9 | ZIP with `../` path in entry name | Refused before read |
| T10 | Overlap synthetic Apple + Strava same timestamp | Conflict surfaced in preview |

Optional later when FIT parser exists: T11 minimal `.fit` fixture generated by code, not
copied from real exports.

Run in CI: `pnpm check:public-data && pnpm test` (and full verify per `AGENTS.md`).

---

## 11. What must land on `main` first

| Prerequisite | Why | Blocking? |
| --- | --- | --- |
| PR [#33](https://github.com/mwhitham/ath/pull/33) npm **1.1.0** publish prep | Aligns shipped CLI with `src/version.ts` **1.1.0**; not required to merge this **plan** | No for plan PR; yes before user-facing Strava **implementation** if help text references 1.1.0 behavior |
| PR [#20](https://github.com/mwhitham/ath/pull/20) Rust foundation approved | Shared FIT/GPX/TCX should live in `ath-core` | Soft gate for implementation: TypeScript-only parser allowed temporarily with removal milestone |
| PR [#31](https://github.com/mwhitham/ath/pull/31) Garmin plan approval | Shared FIT decoder ownership | Coordinate to avoid duplicate parsers |
| `athleticstandard_version` **1.0.0** | Strava route fits existing v1 tables | No format bump required for importer-only work |

This **plan PR** does not depend on #33 merging. The **build PR** should branch from
current `main` after #33 if release numbering matters for `tool_version` strings.

---

## 12. Build PR scope (single PR after approval)

One implementation PR per approved plan (user rule). Deliverables:

| Area | Deliverable |
| --- | --- |
| `ath-core` / `src/import` | Strava bulk parser + shared activity-file decoders |
| CLI | Detection, help, skill CLI regen |
| v1 | `strava_bulk/v1` keys in `provider-import.ts` |
| Tests | §10 fixtures + allowlist hashes |
| Docs | `docs/connections.md` Strava bulk row; `build-history/v1.0.0/progress.md` step |
| Decisions | Append accepted D152+ to `decisions.md` |

**Temporary code:** any TypeScript FIT/TCX bridge deleted when Rust parser ships;
track in `progress.md` with milestone name **Strava FIT cutover**.

**Out of scope for build PR:** Strava API, OAuth, MCP, hosted sync.

---

## 13. Open decisions for review (proposed D152–D163)

These become `decisions.md` entries after approval. Numbers were chosen after checking
`main` (D82–D129), PR #20 (D130–D132), PR #31 (D133–D144 in plan only), and PR #33
(D130–D151 on branch).

| ID | Decision |
| --- | --- |
| **D152** | v1 Strava bulk import uses route `strava_bulk` and `match_key_method` `strava_bulk/v1`. |
| **D153** | Detection requires `activities.csv` and `activities/` together (no lone CSV in v1). |
| **D154** | Primary record type is `workout_session`; series from activity files; CSV fills gaps only. |
| **D155** | Strava `Relative Effort` and similar vendor composites stay out of `provider_score` in v1. |
| **D156** | When CSV and activity file disagree on overlap fields, samples from file win; CSV fills non-sample aggregates. |
| **D157** | Whether to store a content-addressed copy of each original activity file as an attachment on import. |
| **D158** | Whether CSV-only import (no `activities/` folder) is ever accepted. |
| **D159** | Whether saved `routes/*.gpx` from the archive map to v1 records in a later phase. |
| **D160** | Default cross-source workout overlap policy when Strava and Garmin/Apple match (inherit D141 vs new default). |
| **D161** | `Activity Date` parsing: English-only vs ICU/locale list; behavior on parse failure (skip vs refuse entire import). |
| **D162** | Maximum activities per import (same 100k file cap vs separate row cap). |
| **D163** | Implementation lands in `ath-core` before CLI wiring, or temporary TS parser with removal at Rust FIT milestone. |

---

## 14. Open questions (legal and product)

### 14.1 Rights to use bulk export data in a local CLI

**Evidence found (not legal advice):**

| Source | What it says |
| --- | --- |
| [Strava API Policy §6.6 User Bulk Data Export](https://cdn-1a.strava.com/legal/api_policy) | Each user may access and export their own data free of charge through the Bulk Data Export Tool; **nothing in the API Agreement limits that user-facing right**. |
| [Strava Privacy Policy](https://www.strava.com/legal/privacy) | Access/portability right; download instructions linked. |
| [Strava Terms §12 Personal Use](https://www.strava.com/legal/terms) | Services are for **personal use**; business use needs written Strava consent. |
| [Strava API Agreement](https://www.strava.com/legal/api) | Governs **API** access, caching, and display rules — **not** the same code path as a user’s own ZIP from settings. |

**Not found in sources reviewed:** an explicit sentence allowing third-party **desktop
software** to parse the user’s own bulk ZIP. **Not claimed:** that ATH distribution is
automatically permitted. Product positioning: local import of **the athlete’s own**
archive into **their own** SQLite file, no redistribution of Strava data.

**API vs bulk:** API Policy retention limits (for example short-lived API caches) apply
to **Strava Data obtained through the API**, per [API Policy §7](https://cdn-1a.strava.com/legal/api_policy)
and [community clarification on API-sourced data](https://communityhub.strava.com/developers-api-7/clarification-on-api-policy-6-2-retaining-an-athlete-s-own-activity-data-for-historical-comparison-13928).
This importer does **not** call the API.

**Action for owner:** confirm with counsel whether shipping an open-source importer
that reads a user-provided bulk export is acceptable under Terms §12 and API Policy
§6.6 for this project’s distribution model.

### 14.2 Other product questions

1. Should Strava equipment CSVs (`bikes.csv`, `shoes.csv`) create `source_device` rows
   or only string metadata on workouts?
2. Should indoor activities without GPS import as sessions with zero route series?
3. Does `docs/connections.md` need a Strava row before or with the build PR?

---

## 15. Standing rules cross-check

| User-stated rule | Repo source | Plan alignment |
| --- | --- | --- |
| One builder; no legacy-compat layer or engine flags | D117, Rust plan §3 | Single parser path; temporary TS only with removal milestone |
| No real health data in Git | `AGENTS.md`, `CONTRIBUTING.md` | §10 synthetic fixtures only |
| Preserve user data; verified conversion on copies | D124, migration appendix | §9 imports add/revise only; migrations separate |
| Node + iOS first | Rust plan, Garmin plan | §5.3 CLI + `ath-uniffi` |
| rusqlite, SQL migrations, generated contracts | PR #20, `spec/sql/` | Strava logic in `ath-core`; SQL unchanged for importer-only |
| No fallbacks | Rust plan §3 | Refuse or skip with reason; no silent vendor guess |

No conflict found between these rules and `AGENTS.md` / `build-history/README.md`.

---

## 16. References

- Repo: `docs/connections.md`, `SPEC.md` §4.6 and §9.2, `src/v1/schema.ts`,
  `src/import/detect.ts`, `src/db/provider-import.ts`, D91, D94, D108, D110, D112.
- Strava: [bulk export help](https://support.strava.com/en-us/articles/15401919-how-do-i-export-my-strava-data),
  [privacy](https://www.strava.com/legal/privacy), [terms](https://www.strava.com/legal/terms),
  [API agreement](https://www.strava.com/legal/api), [API policy](https://cdn-1a.strava.com/legal/api_policy).
- Related draft plans: PR [#31 Garmin](https://github.com/mwhitham/ath/pull/31),
  PR [#20 Rust](https://github.com/mwhitham/ath/pull/20).
