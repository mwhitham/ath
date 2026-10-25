# Training load from reported effort

Minutes times the effort number the person reported. This reads a personal
report. It is not a measured heart-rate load. If duration or the effort
number is missing, the result says so.

```ath-calculation
name: training-load-effort
duration_field: duration_seconds
effort_field: rating
```
