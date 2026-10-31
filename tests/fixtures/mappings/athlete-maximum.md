# A workout peak aimed at the athlete's maximum

This file must be refused. `maximum_heart_rate_bpm` is the athlete's known maximum.

```ath-mapping
name: athlete-maximum
route: invented_workout
writer_field: source.provider

[workout]
id_field: id
start_field: start_time
end_field: end_time
max_heart_rate_bpm: maximum_heart_rate_bpm bpm bpm
```
