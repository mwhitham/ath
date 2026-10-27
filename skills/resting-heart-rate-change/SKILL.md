---
name: resting-heart-rate-change
description: Subtracts the earlier mean resting heart rate from this record. Use when asking how resting heart rate changed for one source.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply resting-heart-rate-change --record <record-id>.
metadata:
  version: "2.0.0"
---

# Resting heart rate change

This record's resting heart rate minus the mean of earlier resting heart
rate records from the same source.

```ath-calculation
name: same-quantity-change
use: subtract_earlier_mean
```
