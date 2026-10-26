---
name: heart-rate-zones
description: Computes five heart-rate zones from samples and a known maximum heart rate. Use when asking how long one workout spent in each zone.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply heart-rate-zones --record <record-id>.
metadata:
  version: "2.0.0"
---

# Heart-rate zones

Five zones, split at 60, 70, 80, and 90 percent of a known maximum heart rate.
Each sample lasts until the next sample. The last sample lasts as long as the
gap before it. If the maximum is missing, the result says so. Do not guess a
maximum from age.

```ath-calculation
name: heart-rate-zones
sample_field: heart_rate_samples
maximum_field: maximum_heart_rate_bpm
zone: 0.60
zone: 0.70
zone: 0.80
zone: 0.90
```
