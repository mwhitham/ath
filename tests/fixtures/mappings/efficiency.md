# Their efficiency aimed at ours

This file must be refused. Efficiency is computed from `asleep_seconds` and `in_bed_seconds`.

```ath-mapping
name: efficiency
route: invented_sleep
writer_field: source.provider

[sleep]
id_field: id
start_field: start_time
end_field: end_time
efficiency_percent: efficiency
```
