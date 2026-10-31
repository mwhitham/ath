# Junction

Sleep summaries. The provider is `source.provider`. The device is `source.type`, `source.device_id`, and `source.app_id`. `average_hrv` maps to `hrv_rmssd` only with basis `hrv_mean_rmssd`, and only when the writer is oura, whoop_v2, garmin, fitbit, or polar. `duration` stays under `duration`. `efficiency` stays under `efficiency`. There is no activity section.

```ath-mapping
name: junction
route: junction
writer_field: source.provider
sensor: source.type, source.device_id, source.app_id
device_name: source.type
device_model: source.device_id
device_hardware: source.app_id

[sleep]
id_field: id
start_field: bedtime_start
end_field: bedtime_stop
stage_asleep_second: asleep_seconds s s
total: asleep_seconds s s when-absent
stage_awake_second: stage_minutes.awake s min
stage_light_second: stage_minutes.light s min
stage_rem_second: stage_minutes.rem s min
stage_deep_second: stage_minutes.deep s min
hr_resting: resting_heart_rate bpm bpm
respiratory_rate: respiratory_rate brpm brpm
average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd when-writer oura,whoop_v2,garmin,fitbit,polar
score: provider_score.sleep.score score score scale 1-100
recovery_readiness_score: provider_score.recovery_readiness_score score score scale 0-100
```
