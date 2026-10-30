---
name: workout-drift
description: Compare the average of the first half of one workout with the average of the second half. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply workout-drift or ath calculate workout-drift.
metadata:
  version: "2.0.0"
---

# workout-drift

Compare the average of the first half of one workout with the average of the second half. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: workout-drift
needs: quantity
needs: session
allows: source
use: first_half
use: second_half
method: difference between means in equal-duration first and second workout halves
```
