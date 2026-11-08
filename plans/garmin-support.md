# Garmin support plan (proposed)

**Status:** proposed draft for review. **Do not merge this pull request.** After
approval, decisions move to `build-history/v1.0.0/decisions.md` and this file moves
to `build-history/v1.0.0/appendix/garmin-support.md` (see open decision D133).

**Goal:** Let a Garmin wearer load their own recovery and training data into
`athlete.ath.sqlite` without Apple Health, without a hosted sync service, and without
mixing vendor composites into ATH measurements. First release targets **Node (CLI)** and
**iOS (via `ath-uniffi`)** only.

**Checked against `main` at npm **1.1.0** / `ATH_PACKAGE_VERSION` in `src/version.ts`
(release **1.1.0**, PR #25). Record format `athleticstandard_version` remains **1.0.0**.
Latest decision on `main`: **D129**. Draft PR #20 reserves **D130–D132** for Rust
adoption; Garmin open decisions start at **D133**.

---

## 1. Garmin access paths (verified)

### 1.1 Personal API (new, self-service)

Garmin now offers a **Personal API**: a read-only, pull-only key for **one user’s own**
Garmin Connect data. No partner application.

Official sources:

| Topic | Source |
| --- | --- |
| Program overview (Personal tier: Activity JSON & FIT, Health JSON, Women’s Health JSON; pull only) | [Garmin Connect Developer Program overview](https://developer.garmin.com/gc-developer-program/) |
| Key creation, token exchange, 24h bearer tokens, no refresh tokens, key expiry 30d–1y | [Personal API FAQ](https://developer.garmin.com/gc-developer-program/personal/personal-faq/) |
| Token endpoint | `POST https://personal.connectapi.garmin.com/personalapitoken-service/personal-api/personal-api-key/token` with header `Authorization: ApiKey <key>` ([Personal API page](https://developer.garmin.com/gc-developer-program/personal/personal-api/)) |
| Example client and Swagger pointer | [garmin/connect-personal-api-python](https://github.com/garmin/connect-personal-api-python) |

Personal tier **excludes** Courses API, Training API (workout push), Golf API, and
enterprise-only options (ping/pull backfill, enhanced beat-to-beat intervals per
[Health API](https://developer.garmin.com/gc-developer-program/health-api/)).

Garmin’s public docs **do not publish numeric rate limits** for the Personal API. The
build must use conservative concurrency, cache bearer tokens until near expiry (24h max),
and backoff on `429`/`5xx`.

### 1.2 Account data export (offline)

Users request a full archive at Garmin account data management (linked from Garmin
account settings). Delivery is asynchronous (often 24–48 hours; large accounts can take
longer). Garmin does **not** publish a stable, field-level spec for the bulk ZIP layout
in developer docs. The build treats export layout as **discovered structure**, validated
only through **synthetic fixtures** (see §8), not copied real exports.

Observed contents (for planning only; confirm in fixtures during build):

- Activity **FIT** files and JSON activity summaries (`DI-Connect-Fitness/`).
- Wellness JSON summaries and **WELLNESS.fit** monitoring files (`DI-Connect-Wellness/`;
  SpO₂ and other monitoring often appear in wellness FIT per [Garmin forum guidance](https://forums.garmin.com/apps-software/mobile-apps-web/f/garmin-connect-web/421117/where-can-i-find-my-spo2-data)).
- Account metadata (`DI_CONNECT/`).

Health API JSON shapes documented for enterprise exports mirror what the pull client
should parse; field reference:
[Garmin export overview (MyDataHelps)](https://support.mydatahelps.org/garmin-export-overview)
and per-model articles (Daily, Sleep, HRV, Stress Detail, Activity, etc.).

### 1.3 FIT files

[FIT SDK overview](https://developer.garmin.com/fit/overview/): binary activity and
wellness recordings (GPS, HR, power, laps, monitoring). Message definitions live in
the SDK `Profile.xlsx`. Activity FIT arrives via Personal API and account export;
wellness FIT appears in account export monitoring paths.

**Caveat:** Third-party reports say API-delivered FIT may omit some Garmin-computed
fields present in manual downloads. The importer must not invent missing fields; it
imports what the file contains and records gaps in the import receipt.

### 1.4 Recommended order for users

| Order | Path | Role |
| --- | --- | --- |
| **1** | **Account data export** | One-time backfill: full history, all activity FITs, wellness JSON/FIT. No API key. Slow to obtain. |
| **2** | **Personal API pull** | Day-to-day incremental sync after key is stored. Pull-only: user must sync watch to Connect first. |
| **3** | **Loose FIT files** | Ad-hoc activities (USB export, third-party dump). Same parser as export/API FIT. |

Apple Health remains a poor primary path for Garmin wearers (no HRV, no RHR per
`docs/connections.md` and D20).

---

## 2. Fit with Rust adoption (PR #20)

PR [#20](https://github.com/mwhitham/ath/pull/20) defines **`ath-core`** (records,
validation, storage, **provider parsing**, normalization, calculations) with thin
**`ath-node`** and **`ath-uniffi`** (Swift) bindings. The TypeScript CLI keeps args,
help, credentials UI, and model calls.

| Work | Depends on #20? | Notes |
| --- | --- | --- |
| This plan | No | Documentation only. |
| Garmin parsers, FIT decoder, pull HTTP client, dedup, match keys | **Yes** | Lives in `ath-core`; exposed through bindings. |
| `ath pull garmin`, `ath import` for Garmin export | **Yes** | CLI calls core; no second TypeScript rule set. |
| `ath key set garmin` (personal API key) | **Yes** | Store via existing OS secret-store pattern (`src/keyring.ts`, D75); account name `garmin-personal-api`. |
| iOS read/write proof | **Yes** | `apps/verification-ios/` + `ath-uniffi` per rust-adoption §10. |
| Updating `docs/connections.md` | No | After plan approval, with the build PR. |

**Nothing in the Garmin build ships before #20’s foundation proof** (core + bindings +
SQLite round trip). No parallel TypeScript importer and no compatibility layer.

**One build PR** after plan approval + #20 foundation (see §10). No split PRs.

---

## 3. Rules (non-negotiable)

### 3.1 HRV: RMSSD vs SDNN

- ATH stores `hrv_rmssd` and `hrv_sdnn` separately (`src/v1/schema.ts`,
  AGENTS.md). **Never combine them in one baseline.**
- Garmin’s documented overnight field is **`LastNightAvg`** (milliseconds) in
  [HRV Summary export format](https://support.mydatahelps.org/garmin-heart-rate-variability-summary-export-format).
  Garmin describes “beat-to-beat variations” but **does not name RMSSD or SDNN** in
  that spec. Health Snapshot on-device reports RMSSD and SDRR separately; those are
  **not** the same as `LastNightAvg`.
- **Plan rule:** Do not map `LastNightAvg` to `hrv_rmssd` until **D134** is decided
  with evidence from Garmin’s Personal API Swagger or a controlled validation run
  (sanitized, outside Git). Until then, store overnight HRV as
  `provider_score` metric `garmin_last_night_avg` scale `ms` **or** skip the value and
  log `skipped` in the import receipt (see D134).
- Optional `HrvValues` time series (offset map): if beat intervals are present, ATH
  may compute `hrv_rmssd` with a `calculations` block (method, version, window,
  exclusions) per D26 — never label SDNN as RMSSD.
- **Enhanced beat-to-beat** intervals in the Health API are **enterprise-only**; Personal
  API must not assume access.

### 3.2 Vendor scores vs ATH measurements

Garmin composites stay **`provider_score`** (`src/v1/schema.ts`), with `metric`,
`scale`, `source`, and `recorded_at` / provider reporting period (D27 in v0.2.0).

Examples (metric names are provisional until build fixtures lock them):

| Garmin concept | ATH record | Scale |
| --- | --- | --- |
| Body Battery level | `provider_score` `garmin_body_battery` | `0-100` |
| Stress level | `provider_score` `garmin_stress` | `1-100` |
| Sleep score (overall) | `provider_score` `garmin_sleep_score` | device-specific |
| Training Readiness | `provider_score` `garmin_training_readiness` | device-specific |
| Training status label | `provider_score` `garmin_training_status` | categorical string in `value` |
| Recovery time (minutes) | `provider_score` `garmin_recovery_time` | `minutes` |
| HRV status / baseline bands | `provider_score` | per Garmin fields |

ATH **does not** rename these as readiness or recovery in prediction inputs. Any ATH
composite gets a **new name** and an explainable `calculations` entry.

### 3.3 Explainable calculations

Every ATH-derived value (including RMSSD from beats) records: `method`,
`calculation_version`, parameters, input record IDs, window, usable/dropped samples,
and exclusions. Vendor scores never back-fill missing measurements.

---

## 4. Coverage table

**Legend:** ✅ available in source docs or export specs · ⚠️ partial / device-dependent ·
❌ unavailable · 🔒 enterprise-only (not Personal API).

ATH types refer to v1 records in `src/v1/schema.ts`. Series use `measurement_sample`
rows (dense samples in SQLite per D82).

### 4.1 Recovery and wellness

| Garmin metric | Personal API (Health JSON) | Account export | FIT (wellness / monitoring) | Units | Typical sampling | ATH mapping |
| --- | --- | --- | --- | --- | --- | --- |
| Overnight HRV `LastNightAvg` | ✅ HRV Summary ([overview](https://support.mydatahelps.org/garmin-export-overview)) | ✅ `GarminHrvSummary` / wellness JSON | ⚠️ not primary in FIT | ms | 1 value / night | **D134:** `provider_score` or skip until statistic confirmed |
| `LastNight5MinHigh` | ✅ | ✅ | ⚠️ | ms | nightly max over 5 min | `provider_score` `garmin_hrv_5min_high` `ms` |
| `HrvValues` overnight map | ✅ | ✅ samples file | ❌ | ms per offset | sparse overnight | `measurement_sample` `hrv_beats` or interval series; optional derive RMSSD |
| Resting HR (`RestingHeartRateInBeatsPerMinute`) | ✅ Daily Summary | ✅ | ⚠️ | bpm | daily | `point_measurement` `resting_heart_rate` |
| Sleep stages (deep/light/REM/awake) | ✅ Sleep Summary | ✅ `summarizedSleepData.json` | ⚠️ device-dependent | s per stage | per sleep event | `sleep_session` + `source_reported_summaries` |
| Sleep duration | ✅ | ✅ | ⚠️ | s | per event | `sleep_session` |
| Sleep score overall | ✅ | ✅ | ❌ | score | per night | `provider_score` `garmin_sleep_score` |
| Respiration (sleep / all-day) | ✅ Respiration Summary | ✅ | ⚠️ | brpm | epoch / sleep | `point_measurement` `respiratory_rate` and/or series |
| SpO₂ | ✅ Pulse Ox Summary | ✅ (often wellness FIT) | ⚠️ `WELLNESS.fit` | % | epoch / sleep | `point_measurement` `oxygen_saturation` / series |
| Skin temperature deviation | ✅ Skin Temp Summary | ⚠️ | ❌ | °C delta | sleep window | `point_measurement` `temperature_deviation` |
| Stress average / max | ✅ Daily Summary | ✅ | ❌ | 1–100 | daily | `provider_score` `garmin_stress` (daily) |
| Stress time series | ✅ Stress Detail | ✅ samples | ❌ | 1–100 | **3 min** averages ([stress detail spec](https://support.mydatahelps.org/garmin-stress-detail-summary-export-format)) | `provider_score` series via `measurement_sample` or daily side summaries |
| Body Battery time series | ✅ Stress Detail | ✅ samples | ❌ | 0–100 | **3 min** with stress detail | `provider_score` via samples (`garmin_body_battery`) |
| Body Battery charged/drained | ✅ (wellness endpoints / daily battery report) | ⚠️ | ❌ | count | daily | `provider_summary` or `provider_score` per D135 |
| Steps | ✅ Daily / Epoch | ✅ | ⚠️ monitoring | count | daily; epoch **15 min** | `provider_summary` + optional `measurement_sample` `steps` |
| Distance (all-day) | ✅ Daily | ✅ | ⚠️ | m | daily | `provider_summary` / modality distances |
| Floors climbed | ✅ Daily | ✅ | ❌ | count | daily | `provider_summary` |
| Active / BMR calories | ✅ Daily | ✅ | ❌ | kcal | daily | `provider_summary` |
| Intensity minutes | ✅ Daily | ✅ | ❌ | s | daily | `provider_summary` |
| All-day HR samples | ✅ Daily `timeOffsetHeartRateSamples` | ✅ Daily samples | ⚠️ wellness FIT | bpm | **15 s** slots ([daily spec](https://support.mydatahelps.org/garmin-daily-summary-export-format)) | `measurement_sample` `heart_rate` |
| Body composition (weight, BF%, BMI) | ✅ Body Composition | ✅ Index JSON | ❌ | kg, % | per measurement | `point_measurement` `body_weight`, `body_fat_percentage`, etc. |
| Blood pressure | ✅ if device | ⚠️ | ❌ | mmHg | per reading | `point_measurement` systolic/diastolic |
| VO₂ max (run/cycle) | ✅ User Metrics Summary | ⚠️ | ❌ | ml/kg/min | irregular | `point_measurement` `vo2_max` |
| Training readiness / recovery time / training status | ⚠️ User Metrics / Connect-derived | ⚠️ wellness JSON | ❌ | vendor | daily | `provider_score` (Garmin-named) |
| Health Snapshot (RMSSD / SDRR) | ⚠️ Health Snapshot Summary | ⚠️ | ❌ | ms | 2 min session | `point_measurement` `hrv_rmssd` **only if** API labels RMSSD; else `provider_score` |
| Beat-to-beat enhanced intervals | 🔒 Enterprise | ❌ | ⚠️ raw RR in some FIT | ms | beat | `measurement_sample` `hrv_beats` |
| Women’s health (cycle, pregnancy) | ✅ Women’s Health API (Personal tier) | ⚠️ | ❌ | varies | per event | **D136:** new record types or skip v1 |
| Steps in wheelchair mode | ✅ Daily pushes fields | ⚠️ | ❌ | pushes | daily | `provider_summary` (optional) |

### 4.2 Workouts and activities

| Garmin metric | Personal API (Activity) | Account export | Activity FIT | Units | Sampling | ATH mapping |
| --- | --- | --- | --- | --- | --- | --- |
| Activity summary (sport, duration, distance, HR min/max/avg) | ✅ Activity Summary JSON | ✅ `summarizedActivities.json` | ✅ session message | mixed | per activity | `workout_session` |
| Activity details (laps, samples) | ✅ Activity Details JSON | ⚠️ | ✅ lap + record messages | mixed | per lap; HR/GPS **~1 Hz** typical | `workout_session.segments` + samples |
| Full activity FIT download | ✅ JSON & FIT ([overview](https://developer.garmin.com/gc-developer-program/)) | ✅ per-activity FIT | ✅ (same file) | mixed | record timestamps | parse FIT → session + `measurement_sample` |
| GPS track | ✅ details / FIT | ✅ FIT | ✅ position | deg / m | ~1 Hz | samples + route attachment **D137** |
| Power, cadence, speed, elevation | ✅ details / FIT | ✅ FIT | ✅ | W, rpm, m/s, m | per record | `measurement_sample` quantities |
| Move IQ auto activities | ✅ MoveIQ summary | ⚠️ | ❌ | — | event | `workout_session` with `activity_type` auto |
| Manually edited Connect activities | ✅ manual summary type | ⚠️ | ❌ | — | — | `workout_session` (lower confidence in receipt) |
| Garmin Training Effect / performance condition / sweat loss | ⚠️ may be stripped in API FIT | ⚠️ in manual FIT only | ⚠️ | vendor | per activity | `provider_score` if present; **never invent** |
| HYROX / station splits | ❌ as first-class | ❌ | ⚠️ via lap/user fields | — | lap | segments from FIT laps **D138** |
| Courses / workouts pushed to device | ❌ Personal tier | ❌ | ❌ | — | — | out of scope |

### 4.3 Unavailable on any planned path

| Item | Reason |
| --- | --- |
| Live webhook / push feed | Personal tier is pull-only ([overview](https://developer.garmin.com/gc-developer-program/)) |
| OAuth multi-user app | Out of scope (single-user personal key) |
| Android / Watch / hosted sync | Explicitly not blockers for this release |
| SDNN from Garmin overnight HRV | Not documented; Terra reports `avg_hrv_sdnn` empty for Garmin ([Terra docs](https://docs.tryterra.co/help-center/help-topics/data-api-sdk/provider-coverage-and-per-provider-data-availability/garmin-sleep-hrv-resting-hr-null)) |

---

## 5. Credentials, sync, dedup, and conflicts

### 5.1 Storing the personal API key

- Command: extend `ath key` with account **`garmin`** (same `KEYRING_SERVICE`
  `athleticstandard` in `src/keyring.ts`; never write the key into `.ath.sqlite`).
- CLI obtains a bearer token via Garmin’s token endpoint; cache token in memory for the
  process; optional encrypted cache file is **D139** (default: re-fetch each run).
- Warn before key expiry (30d–1y); document rotation in help text.

### 5.2 Pull behavior (`ath pull garmin`)

- **Incremental cursor** stored in the database (new extension table or
  `source_device` metadata — **D140**): last successful calendar dates per feed
  (dailies, sleep, HRV, stress, activities).
- Pagination: follow Garmin list endpoints until empty; respect backoff when rate
  limited.
- **Idempotent imports:** reuse provider import match keys
  (`record_import_keys` in `src/db/provider-import.ts`): stable Garmin `summaryId` /
  activity id + record type + quantity.
- **Revisions:** changed Garmin `InsertedDate` or revised summary → new revision per
  existing import rules; never silent overwrite.
- **Receipt** (Rust plan §8): imported / unchanged / skipped / invalid with reasons.

### 5.3 Overlap with Apple Health, WHOOP, Oura

| Overlap | Rule |
| --- | --- |
| Same workout from Garmin + Apple Health | Different `source` → both may exist; match on start time + type → **conflict** or skip per `collision_group` policy **D141** |
| RHR / sleep from Garmin vs WHOOP/Oura | Separate sources; do not merge; baselines stay per source |
| HRV RMSSD from WHOOP/Oura vs Garmin overnight | Different statistics possible; keep separate; never baseline together |
| Apple `hrv_sdnn` vs Garmin | Never interchangeable |

Default: **keep all sources**; predictions use per-source baselines (D31). Optional
user preference to prefer one source per quantity is **D142**.

---

## 6. Data safety

- No real exports in Git. Fixtures under `tests/fixtures/exports/garmin/` must be
  synthetic and listed in `scripts/check-public-data.ts`.
- Import and pull commands never modify schema without `ath migrate` rules (D124).
- Existing databases: imports only add/revise records; no automatic erase.
- Validation evidence in decisions omits dates, routes, device serials, and sample values.

---

## 7. Verification (report later)

Run after the build PR lands. Mark each **PASS**, **FAIL**, **BLOCKED**, or
**NOT IMPLEMENTED**.

| ID | Check | Command / environment |
| --- | --- | --- |
| V1 | Public data allowlist | `pnpm check:public-data` |
| V2 | Types and unit tests | `pnpm typecheck && pnpm test` |
| V3 | Build | `pnpm build` |
| V4 | Synthetic Garmin export import | `ath import tests/fixtures/exports/garmin/...` → preview + apply |
| V5 | Synthetic FIT-only import | `ath import` on fixture FIT directory |
| V6 | Pull with mock HTTP | Rust tests with recorded fixtures (no network in CI) |
| V7 | Match-key reimport unchanged | Second import → `matching_records` only |
| V8 | Changed Garmin summary → revision | Fixture with newer `InsertedDate` |
| V9 | HRV rule | No `hrv_rmssd` without D134 decision + evidence |
| V10 | Vendor score scales | `ath check` on fixture DB |
| V11 | RMSSD/SDNN baseline separation | `ath stats` / calculate baselines per source |
| V12 | Key not in DB | `ath key set garmin` then inspect SQLite |
| V13 | iOS simulator proof | `apps/verification-ios/`: open DB, import or pull stub, correct record; **BLOCKED** on Linux CI |
| V14 | UniFFI round trip | Swift test calls same core import as CLI |
| V15 | Multi-source conflict | Garmin + Apple synthetic overlap → expected conflict |

---

## 8. Open decisions for Mike

These become D-entries after approval. **D130–D132** remain reserved for PR #20.

| ID | Decision |
| --- | --- |
| **D133** | Where the approved plan text lives: move `plans/garmin-support.md` → `build-history/v1.0.0/appendix/garmin-support.md` on approval (conflicts with draft-only `plans/` until then — see PR note). |
| **D134** | How to store Garmin `LastNightAvg` until Garmin names the statistic (skip vs `provider_score` vs `hrv_rmssd` with evidence). |
| **D135** | Map Body Battery “charged/drained” integers vs time series scores. |
| **D136** | Import Women’s Health API in v1 or defer. |
| **D137** | Store GPS as `measurement_sample` only vs optional route `attachment`. |
| **D138** | Lap-driven HYROX/segment labels from FIT (heuristic vs manual). |
| **D139** | Bearer token cache on disk vs in-memory only. |
| **D140** | Cursor storage for incremental `ath pull garmin`. |
| **D141** | Default when two sources match the same workout (keep both vs prefer Garmin). |
| **D142** | User-facing source preference for overlapping metrics. |
| **D143** | Minimum Personal API surface for v1 (sleep+HRV+RHR+stress+body battery+dailies+activities) vs full Health API parity. |
| **D144** | Account export detection marker (ZIP paths) vs explicit `ath import garmin-export/`. |

---

## 9. Build PR scope (single PR)

**Prerequisites:** PR #20 foundation proof passed; this plan approved.

| Area | Deliverable |
| --- | --- |
| `ath-core` | Garmin JSON parsers (Health + Activity), FIT decoder, normalization, match keys, pull client |
| `ath-node` / `ath-uniffi` | Binding coverage for import + pull |
| CLI | `ath import <garmin-export>`, `ath pull garmin`, `ath key set garmin`, help + `pnpm generate:skill-cli` |
| Fixtures | Synthetic export ZIP, FIT, and HTTP replay files + `check-public-data` entries |
| Docs | Update `docs/connections.md`, `SPEC.md` if needed, `build-history/v1.0.0/progress.md` |
| Decisions | Append approved D133–D144 (or subset) to `decisions.md` |

**Temporary code:** none intended. If a short-lived `_tmp` bridge is unavoidable during
#20 migration, name it with removal at **Garmin cutover** milestone in `progress.md` —
no long-lived TypeScript parser.

**Out of scope for the build PR:** Open Wearables connector, Android, hosted sync,
Training/Courses push, MCP, multi-athlete.

---

## 10. References

- Repo: `docs/connections.md`, D20, D27, D31, `src/db/provider-import.ts`,
  `src/v1/schema.ts`, `src/keyring.ts`, `skill/references/cli.md`
- Garmin: [Developer Program overview](https://developer.garmin.com/gc-developer-program/),
  [Personal FAQ](https://developer.garmin.com/gc-developer-program/personal/personal-faq/),
  [Health API features](https://developer.garmin.com/gc-developer-program/health-api/),
  [FIT SDK](https://developer.garmin.com/fit/overview/),
  [connect-personal-api-python](https://github.com/garmin/connect-personal-api-python)
- Export field models: [Garmin export overview (MyDataHelps)](https://support.mydatahelps.org/garmin-export-overview)
- Rust direction: PR [#20](https://github.com/mwhitham/ath/pull/20),
  `build-history/v1.0.0/appendix/rust-adoption.md` (on draft branch)
