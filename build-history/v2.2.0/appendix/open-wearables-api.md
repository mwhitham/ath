# Open Wearables, as checked on 6 November 2026

This study is the evidence for the 2.2.0 plan. The rules are in
[the specification](../spec.md) and [the decisions](../decisions.md). Provider
software changes. A fact below is tied to the page or the file it came from.
Nothing here was read from a live server, and no key was used.

## What the product is

Open Wearables is the program in the public repository
`the-momentum/open-wearables`. The repository describes it as a self-hosted
platform that puts wearable data behind one API. The license file reported by
GitHub is MIT. The repository was created on 30 October 2025. GitHub reported
the default branch `main` pushed at 13:43 UTC on 6 November 2026.

The newest published release on that date was `0.11.0`, published at 16:10 UTC
on 2 November 2026, at commit `183e8cc4f6de`. Its title is "0.11 - Daily
aggregates & faster summaries". The commit on `main` that day,
`7ca121dd4e48`, is "chore: update OW to 1.0 (#2065)". There was no GitHub
release named 1.0.0.

The GitHub homepage field is `https://docs.openwearables.io`. This study did
not fetch that host. The pages it did fetch are on
`https://openwearables.io/docs`. The API introduction there says the reference
is generated from `docs/openapi.json` on every change, so the website describes
current `main`, and a self-hosted copy should be read from that copy's own
`/openapi.json`.

Checked:

- Repository: `https://github.com/the-momentum/open-wearables`
- Release `0.11.0`: `https://github.com/the-momentum/open-wearables/releases/tag/0.11.0`
- API introduction, fetched 6 November 2026: `https://openwearables.io/docs/api-reference/introduction`
- Integration guide, fetched the same day: `https://openwearables.io/docs/dev-guides/integration-guide`
- Doc index: `https://openwearables.io/docs/llms.txt`
- OpenAPI at the `0.11.0` tag: `docs/openapi.json`, `info.version` `0.11.0`
- OpenAPI on `main` the same day: `docs/openapi.json`, `info.version` `1.0.0`

The integration guide says "self-hosted or cloud". In the same guide the cloud
case is a deployment the reader runs. This study did not find a separate
hosted service, in the repository description, the `0.11.0` notes, or the API
introduction, that stores data for people who do not run the server.

Open Wearables is not Junction. Junction remains the other shipped mapping
file, and this plan does not call it.

## The read this plan uses

All three reads are under `/api/v1` and take a user id in the path. The user
id is an Open Wearables UUID. Dates are required.

| Read | Path on `0.11.0` | Page shape |
|---|---|---|
| Sleep | `GET /api/v1/users/{user_id}/events/sleep` | `data`, `pagination`, `metadata` |
| Workouts | `GET /api/v1/users/{user_id}/events/workouts` | `data`, `pagination`, `metadata` |
| Timeseries | `GET /api/v1/users/{user_id}/timeseries` | `data`, `pagination`, `metadata` |

`pagination.has_more` is required in the OpenAPI schema. `next_cursor` is
null when there is no next page. The cursor code in `0.11.0` is a URL-safe
base64 of a timestamp, an id, and an optional `prev_` prefix
(`backend/app/utils/pagination.py`). `limit` defaults to 50 and the maximum
is 1000.

Sleep and timeseries take `filter_by_priority`, default `false`. The `0.11.0`
sleep handler passes that flag into the query. The `0.11.0` timeseries handler
does the same. On `main` (OpenAPI `1.0.0`, not a release), an omitted
`source_selection` means all devices when `filter_by_priority` is false. The
`main` workout list uses all devices when `source_selection` is omitted. The
`0.11.0` workout list has no priority flag. It returns the workouts in range.

Timeseries `resolution` defaults to `raw`. The enum is `raw`, `1min`, `5min`,
`15min`, `1hour`, `1day`, `1week`, `1month`. The `0.11.0` service copies that
value into `metadata.resolution` (`timeseries_service.py` around the
`TimeseriesMetadata` construction).

The `0.11.0` OpenAPI parameter `types` is an array of `SeriesType`. It omits
`style` and `explode`. For a query parameter, OpenAPI's default style is
`form`, and the default explode for `form` is true, which is one query
parameter per value. A comma-joined string is not a member of that enum.
This study did not send the request. The plan sends one parameter per type.

The `0.11.0` release notes say day and session values are no longer samples.
They are read with `resolution=1day` or from the sleep or workout. The notes
say a sample row can carry `window_seconds`, and an aggregate can carry
`computed_by` of `provider` or `open_wearables`. The raw sample builder in
`timeseries_service.py` (the `_to_sample` call in the raw return) does not
pass `computed_by`. The field exists on the schema and can be null.

`ComputedBy` is `provider` or `open_wearables`. The description says who
computed an aggregate.

## Fields the mapping already names

Sleep, from the `0.11.0` schema `SleepSession`: `id`, `start_time`,
`end_time`, `zone_offset`, `source`, `duration_seconds`,
`sleep_duration_seconds`, `time_in_bed_seconds`, `efficiency_percent`,
`stages`, `sleep_stage_intervals`, `is_nap`. Stages have `awake_minutes`,
`light_minutes`, `deep_minutes`, `rem_minutes`. `source` has `provider`,
`source`, `device`, `device_type`, `device_id`, and read-only `device_name`.

Workouts, from `Workout`: `id`, `type`, `name`, `start_time`, `end_time`,
`zone_offset`, `source`, `entry_source`, `intensity`, the distance, heart-rate,
and elevation fields the mapping already names, plus fields it does not name.
`entry_source` is `manual`, `automatic`, or `unknown`. The schema text says
`manual` is a self-reported entry with no device measurement behind it.

Timeseries, from `TimeSeriesSample`: `timestamp`, `zone_offset`, `type`,
`value`, `unit`, `source`, `is_daily_total`, `window_seconds`, `period_end`,
`computed_by`, `stats`.

`start_time` and `timestamp` are schema format `date-time`. This study did not
capture a live JSON body, so it does not claim the wire format beyond that
schema. `zone_offset` is a separate string, `+01:00` or `-05:30`, or null.

The shipped mapping file's timeseries lines do not name `heart_rate`. In
Athletic Standard, `heart_rate` is a series quantity
(`SERIES_QUANTITY_UNITS` in `src/v1/schema.ts`). It is not one of the point
quantities the mapping engine writes. A raw heart-rate stream therefore has
nowhere to go in the current mapping.

## Time bounds

The API introduction says a date-only start is midnight UTC and a date-only
end covers that whole UTC day.

The `0.11.0` parser does not say UTC. `parse_query_datetime` in
`backend/app/utils/dates.py` uses `datetime.fromisoformat` after rejecting a
value that parses as a number. A date-only string becomes midnight with no
timezone. `parse_query_end_datetime` turns a date-only end into the next
midnight, also with no timezone, because the comparison is `<`. This study
did not follow those naive values into the database session, so it does not
claim which timezone PostgreSQL applies.

A full timestamp is sent through as parsed. The docs say a value with an
explicit time is used as given, and `end_time` at midnight includes nothing
later that day.

## Credentials

Data routes depend on `ApiKeyDep` (`backend/app/services/api_key_service.py`
on `0.11.0`). That dependency accepts a developer login JWT or the header
`X-Open-Wearables-API-Key`. A missing or unknown key is HTTP 401, detail
"Authentication required: provide JWT token or API key" or "Invalid or
missing API key".

The integration guide says the key is not a bearer token, and
`Authorization: Bearer` with the API key returns 401. The OpenAPI operation
security lists `OAuth2PasswordBearer` and marks the API-key header optional.
Both can be true: the header is optional in the schema because a developer
JWT is the other way in. The header is how an integration authenticates.

`POST /api/v1/users/{user_id}/token` returns a token the `0.11.0` route text
limits to SDK endpoints. `get_current_developer` in `backend/app/utils/auth.py`
rejects a token whose scope is `sdk` on developer routes. There is no
user-scoped token for the sleep, workout, and timeseries reads. The API key
is an administrator key for the whole deployment. The integration guide and
the credentials page say that.

`GET /api/v1/users` lists every user and uses page/limit pagination, not the
cursor used by the three data reads. `GET /api/v1/users/{user_id}/personal-record`
returns birth date, gender, and sex.

`DELETE` exists for one sleep, one workout, one provider connection, and one
provider's stored data. Those are writes against the server. None of the
three read routes takes `updated_since` or returns a deletion list. Sync
routes (`/sync`, `/sync/historical`, `/sync/runs`, `/sync/stream`) report or
start provider sync. They are not a feed of changed records.

`GET /api/v1/config` returns feature flags and requires a developer JWT. It
has no version field. The docs point at `/openapi.json` for the running
version. That file is the full specification (about 600 KB at `0.11.0` and
about 700 KB on `main` the day of this study).

Health scores use `limit` and `offset`. The `0.11.0` notes say Open Wearables
stops computing its own scores on 6 November 2026. Recovery summaries are
daily objects with a `date` and no instant. They include `avg_hrv_rmssd_ms`
and `avg_hrv_sdnn_ms` as separate fields.

No rate-limit field was in the API introduction or in the three `0.11.0`
route files. A limit may still exist in middleware this study did not open.

## What was not checked

- A running server's JSON, including the exact `start_time` text.
- Whether Docker tag `latest` matches release `0.11.0` or `main`.
- The database timezone applied to a date-only bound.
- Middleware rate limits.
- The host `https://docs.openwearables.io`.
