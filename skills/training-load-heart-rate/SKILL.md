---
name: training-load-heart-rate
description: Adds training load from heart-rate samples, resting heart rate, and maximum heart rate. Use when asking for heart-rate training load on one workout.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply training-load-heart-rate --record <record-id>.
metadata:
  version: "2.0.0"
---

# Training load from heart rate

For each sample, reserve is (heart rate - resting heart rate) / (maximum -
resting heart rate). Add duration in minutes times reserve times e to the
power of factor times reserve. Harder minutes count more. If resting heart
rate, maximum heart rate, or samples are missing, the result names the
missing one.

```ath-calculation
name: training-load-heart-rate
sample_field: heart_rate_samples
rest_field: resting_heart_rate_bpm
maximum_field: maximum_heart_rate_bpm
factor: 1.92
```
