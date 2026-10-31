# LastNightAvg aimed at RMSSD

This file must be refused. `LastNightAvg` does not name the statistic.

```ath-mapping
name: last-night-rmssd
route: invented_hrv
writer_field: source.provider

[timeseries]
time_field: timestamp
value_field: value
type.LastNightAvg: hrv_rmssd ms ms
```
