---
name: resting-heart-rate
description: Finds the lowest heart rate held for one continuous minute on one record. Use when asking for resting heart rate from samples.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply resting-heart-rate --record <record-id>.
metadata:
  version: "2.0.0"
---

# Resting heart rate

The lowest heart rate held for one continuous minute during the samples on
this record. A shorter dip does not count. A value supplied by the source
stays on the record under its own field. This calculation is separate.

```ath-calculation
name: resting-heart-rate
use: lowest_held
sample_field: heart_rate_samples
sustain_ms: 60000
```
