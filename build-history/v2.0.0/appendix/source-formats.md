# Open Wearables, Junction, and Athletic Standard

Checked against the public docs on 27 October 2026. Provider docs change.
The pages are listed at the end. This study is the basis for D161.

## What each one is

Open Wearables is a server a person runs. It signs in to device makers, receives their data, and stores its own objects. The usual read is those objects. Sleep comes back as one session object. A workout comes back as one workout object. A measurement comes back as a time series. The original message from the device maker can be saved for debugging. That save is off unless someone turns it on. It is not the API a program reads to build a record.

Junction is a hosted service that does the same job. Its summary endpoints return Junction's objects. Sleep, activity, and workouts each have a summary. A separate raw endpoint returns what the provider sent, wrapped in Junction's envelope. Some summary numbers are calculated by Junction from samples. They are not always a copy of a field the device stored.

Athletic Standard is the file on the machine. A device reading and a personal report are different records. A personal report cannot claim a source. An Oura day stays an Oura day. A WHOOP cycle stays a WHOOP cycle. They are not turned into a midnight-to-midnight day. RMSSD and SDNN stay separate. A field that is not one of ours stays in `supplied_fields` under the name it arrived with. The original file is not stored. If two paths describe one workout, both records stay. A calculation uses one of them.

Open Wearables and Junction are ways to get data in. They are not a replacement for the record.

## What a response contains

Open Wearables sleep uses `sleep_duration_seconds`, `time_in_bed_seconds`, `efficiency_percent`, and stage minutes (`awake_minutes`, `light_minutes`, `deep_minutes`, `rem_minutes`). The provider is `source.provider`, such as `garmin` or `apple`. Workouts use `type`, `name`, `duration_seconds`, `distance_meters`, `avg_heart_rate_bpm`, and `max_heart_rate_bpm`. Heart-rate zones are omitted unless the request asks for them. Timeseries names the statistic. `heart_rate_variability_rmssd` and `heart_rate_variability_sdnn` are different types, both in milliseconds. Their docs record a correction: Oura's RMSSD was once stored under the SDNN type. New Oura samples are stored as RMSSD. Older rows needed a separate relabel. That is a reason to keep the provider on every record.

Junction sleep summaries use `bedtime_start`, `bedtime_stop`, `duration`, stage seconds, `efficiency`, `hr_resting`, and `average_hrv`. Their query layer documents `average_hrv` as `hrv_mean_rmssd`. It also has a separate column, `hrv_mean_sdnn`. The summary field the API returns is still named `average_hrv`. Activity summaries use `steps`, `calories_active`, `calories_total`, and `heart_rate.resting_bpm`. The provider is `source.slug`. The raw sleep endpoint returns `data` as the provider's own JSON, plus Junction's user id and timestamp. Missing stage values are `null` on the newer stage-second fields. Older fields returned `0` when the provider did not send a stage. Those older fields are deprecated in their docs.

## Field comparison

| Meaning | Open Wearables | Junction summary | Athletic Standard |
|---|---|---|---|
| Time asleep | `sleep_duration_seconds` | `stage_asleep_second`, also the older `total` | `asleep_seconds` on a sleep session |
| Time in bed | `time_in_bed_seconds` | `duration` is the session length, not always time in bed | `in_bed_seconds` |
| Sleep stages | stage minutes on the session | `stage_light_second`, `stage_rem_second`, `stage_deep_second`, `stage_awake_second` | `stage_minutes` |
| Sleep efficiency | `efficiency_percent` | `efficiency` | calculated from asleep and in bed, not stored as their percentage unless they sent one |
| RMSSD | `heart_rate_variability_rmssd` | `average_hrv`, documented as `hrv_mean_rmssd` | `hrv_rmssd` only when the source says RMSSD |
| SDNN | `heart_rate_variability_sdnn` | `hrv_mean_sdnn` in the query layer | `hrv_sdnn` only when the source says SDNN |
| Unlabelled HRV | not their usual timeseries name | a raw provider field that only says HRV | `supplied_fields` under that name. `LastNightAvg` stays `LastNightAvg` |
| Resting heart rate | `resting_heart_rate` | `hr_resting` or `heart_rate.resting_bpm` | `resting_heart_rate` |
| Workout | `type`, `duration_seconds`, `distance_meters` | workout summary fields, provider named on `source.slug` | workout session, with route and samples when we have them |
| Provider score | recovery, strain, readiness, body battery as their scores | `score`, `recovery_readiness_score` | `provider_score`. It stays that provider's score |
| Original names | the debug payload, off by default | the `/raw` endpoint | `supplied_fields` on the record we keep |
| Who measured it | `source.provider` | `source.slug` | a source record. The path is part of the source, so Garmin through Open Wearables is not the same source as a Garmin file |

A percentage from them is not stored as if we had divided asleep by in bed ourselves. If they send the two durations, those are the stored fields. Their efficiency can be kept under their field name.

## How a pull writes our records

A pull reads one service's summary objects for a day or a range. The key stays in the password store. It is not written into the database. The same rule already covers the Garmin personal key.

A shipped mapping file names their field and our field. A mapped field becomes the typed value. A field that is not in the mapping stays in `supplied_fields` under their name. Their record id is the match key, so a second pull updates the same record instead of adding another. The source names the device and the path.

Junction's raw endpoint is not the default pull. Using it means reading that provider's own JSON, which is a provider reader, not the summary mapping. Open Wearables' debug storage is not a pull.

Both records stay when a file import and a pull describe the same workout. A calculation uses one record. Run it again with the other record to see the other device.

## A mapping file for a new source

A new source does not get a new record format. Something still has to turn the bytes into named fields. An Apple export, a FIT file, a GPX file, and a CSV need a reader, because a mapping file does not parse those bytes. After the fields have names, the mapping file is enough.

The file lists their field and our field. A person can add a file for a source we do not ship. The program reads it on import the way it reads a calculation skill.

The mapping cannot turn a personal report into a measured record. A measured record needs a source. A personal report cannot claim one. The mapping cannot rename a generic HRV field to `hrv_rmssd` or `hrv_sdnn`. Open Wearables' two HRV types can be mapped, because the type name says which statistic it is. Junction's `average_hrv` can be mapped to `hrv_rmssd` only because their own query docs define that column as RMSSD. A field that only says HRV stays under its own name.

The shipped mappings, when the pull is built, are Open Wearables summaries and Junction summaries. The pull and the mapping file are not built in the change that records this study.

## Pages checked

- Open Wearables sleep: `https://openwearables.io/docs/api-reference/external:-events/list-sleep-sessions`
- Open Wearables workouts and types: `https://openwearables.io/docs/architecture/data-types`
- Open Wearables raw payload storage: `https://openwearables.io/docs/dev-guides/raw-payload-storage`
- Open Wearables Oura HRV relabel: `https://openwearables.io/docs/data-migrations/relabel-oura-hrv-sdnn-to-rmssd`
- Junction sleep summary and raw: `https://docs.junction.com/api-reference/data/sleep/get-summary` and `https://docs.junction.com/api-reference/data/sleep/get-raw`
- Junction activity summary and raw: `https://docs.junction.com/api-reference/data/activity/get-summary` and `https://docs.junction.com/api-reference/data/activity/get-raw`
- Junction column names, including `average_hrv` as `hrv_mean_rmssd`: `https://docs.junction.com/sense/query-dsl/column-expressions`
- Junction calculated fields: `https://docs.junction.com/wearables/providers/metric-sources-and-limitations`
