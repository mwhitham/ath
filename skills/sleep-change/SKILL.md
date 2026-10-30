---
name: sleep-change
description: Subtracts the earlier mean time asleep from this night. Use when asking how sleep duration changed for one source.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-change --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep change

This night's time asleep minus the mean of earlier nights from the same
source. This is separate from heart-rate variability and resting heart rate.

```ath-calculation
name: sleep-change
use: subtract_earlier_mean
field: asleep_seconds
```
