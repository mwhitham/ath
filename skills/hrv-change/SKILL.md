---
name: hrv-change
description: Subtracts the earlier mean of the same heart-rate variability statistic from this record. Use when asking how RMSSD or SDNN changed. The two statistics stay separate.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply hrv-change --record <record-id>.
metadata:
  version: "2.0.0"
---

# Heart-rate variability change

This record's value minus the mean of earlier records with the same
statistic and the same source. RMSSD and SDNN stay separate because the
record's quantity is part of the comparison. There is no combined recovery
score.

```ath-calculation
name: same-quantity-change
```
