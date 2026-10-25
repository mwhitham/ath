# Resting heart rate

The lowest heart rate held for one continuous minute during the samples on
this record. A shorter dip does not count. A value supplied by the source
stays on the record under its own field. This calculation is separate.

```ath-calculation
name: resting-heart-rate
sample_field: heart_rate_samples
sustain_ms: 60000
```
