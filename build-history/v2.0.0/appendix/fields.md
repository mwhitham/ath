# Fields this version understands

On a workout, these names are stored as their own values when the import sees
them: `name`, `description`, `text_result`, `activity_type`, `start_at`,
`end_at`, `maximum_heart_rate_bpm`, `resting_heart_rate_bpm`,
`heart_rate_samples`, `sets`, and `route`.

Garmin JSON uses `activityName`, `description`, `activityType`,
`startTimeGMT`, `duration`, `maxHR`, `samples`, and `route` for those values.
`maximum_heart_rate_bpm` is the athlete's known maximum, not the highest
sample. FIT and GPX do not set it. Garmin JSON `maxHR` is that activity's
peak and is stored in the field when present (D172).
Strava CSV uses `Activity Name`, `Activity Type`, `Activity Date`,
`Elapsed Time`, `Activity Description` or `Notes`, and a GPX file named in
`Filename` for the route and heart-rate samples.

Every other field on that record is kept in `supplied_fields` under the
original name. Garmin `Feeling` and Strava `Gear` are examples. Garmin
`LastNightAvg` is stored as a provider summary with that name and the unit
`ms`. It is not stored as `hrv_rmssd` or `hrv_sdnn`.

Files that are not training or recovery records are listed under `left_out`.
The original file is not saved in the database.
