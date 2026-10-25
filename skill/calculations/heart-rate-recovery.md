# Heart-rate recovery

Heart rate at the end of the effort minus the heart rate one minute later.
The one-minute sample is the sample at the end offset plus 60000 milliseconds.
If that sample is missing, the result says so.

```ath-calculation
name: heart-rate-recovery
sample_field: heart_rate_samples
later_offset_ms: 60000
```
