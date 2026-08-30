# Fruity Compressor

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **6**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Threshold | 0.0 dB (1) | -60.0 dB | -45.0 dB | -30.0 dB | -15.0 dB | 0.0 dB | yes |
| 2 | Ratio | 1.0 : 1 (0.0203) | 0.4 : 1 | 7.8 : 1 | 15.2 : 1 | 22.6 : 1 | 30.0 : 1 | yes |
| 3 | Gain | 0.0 dB (0.5) | -30.0 dB | -15.0 dB | 0.0 dB | 15.0 dB | 30.0 dB | yes |
| 4 | Attack | 15.0 ms (0.0375) | 0.0 ms | 100.0 ms | 200.0 ms | 300.0 ms | 400.0 ms | yes |
| 5 | Release | 200 ms (0.0498) | 1 ms | 1001 ms | 2001 ms | 3000 ms | 4000 ms | yes |
| 6 | Type | Hard (0) | Hard | Vintage | Hard/R | Medium/R | Soft/R | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
