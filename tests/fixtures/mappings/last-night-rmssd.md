# LastNightAvg aimed at RMSSD

This file must be refused. `LastNightAvg` does not name the statistic.

```ath-mapping
name: last-night-rmssd
route: invented_hrv
writer_field: source.provider
value_basis: provider

[timeseries]
time_field: timestamp
type_field: type
value_field: value
unit_field: unit
type.LastNightAvg: hrv_rmssd ms ms
```
