---
name: heart-rate-recovery
description: Computes the drop in heart rate one minute after an effort. Use when asking how fast heart rate fell after one workout.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply heart-rate-recovery --record <record-id>.
metadata:
  version: "2.0.0"
---

# Heart-rate recovery

Heart rate at the end of the effort minus the heart rate one minute later.
The one-minute sample is the sample at the end offset plus 60000 milliseconds.
If that sample is missing, the result says so.

```ath-calculation
name: heart-rate-recovery
use: value_after_end
sample_field: heart_rate_samples
later_offset_ms: 60000
```
