# Open Wearables

Sleep sessions, workouts, and timeseries samples. The provider is `source.provider`. The device is `source.device_type`, `source.device`, and `source.source`. Stage minutes are under `stages`. `efficiency_percent` stays under that name. `entry_source` `manual` is left out. `intensity` stays under that name. A timeseries type is stored only when a line below names it.

```ath-mapping
name: open-wearables
route: open_wearables
writer_field: source.provider
sensor: source.device_type, source.device, source.source
device_name: source.device_name, source.device
device_model: source.device
device_hardware: source.device_id

[sleep]
id_field: id
start_field: start_time
end_field: end_time
sleep_duration_seconds: asleep_seconds s s
time_in_bed_seconds: in_bed_seconds s s
stages.awake_minutes: stage_minutes.awake min min
stages.light_minutes: stage_minutes.light min min
stages.deep_minutes: stage_minutes.deep min min
stages.rem_minutes: stage_minutes.rem min min

[workout]
id_field: id
start_field: start_time
end_field: end_time
type: activity_type
name: name
distance_meters: source_reported_summaries.distance m m
elevation_gain_meters: source_reported_summaries.elevation_gain m m
avg_heart_rate_bpm: provider_summary.average_heart_rate bpm bpm basis provider_or_open_wearables
max_heart_rate_bpm: provider_summary.max_heart_rate bpm bpm basis provider_or_open_wearables

[timeseries]
time_field: timestamp
value_field: value
type.heart_rate_variability_rmssd: hrv_rmssd ms ms
type.heart_rate_variability_sdnn: hrv_sdnn ms ms
type.resting_heart_rate: resting_heart_rate bpm bpm
type.respiratory_rate: respiratory_rate brpm brpm
type.skin_temperature: skin_temperature celsius °C
type.skin_temperature: wrist_temperature_sleeping celsius °C when-writer apple
type.oxygen_saturation: oxygen_saturation percent %
type.heart_rate_recovery_one_minute: hr_recovery bpm bpm
type.vo2_max: provider_summary.vo2_max ml_kg_min ml/kg/min
type.garmin_stress_level: provider_score.garmin_stress_level score score scale 1-100
type.garmin_body_battery: provider_score.garmin_body_battery percent score scale 0-100
```
