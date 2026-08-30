# Spreader

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **5**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Spread | 50.0% (0.5) | 0.0% | 25.0% | 50.0% | 75.0% | 100.0% | yes |
| 2 | Stereo separation | Original (0.5) | 100% merged | 50% merged | Original | 50% separated | 100% separated | yes |
| 3 | Low-Frequency Bypass | 10.00 Hz (0.01) | 0.00 Hz | 250.00 Hz | 500.00 Hz | 5250.00 Hz | 10000.00 Hz | yes |
| 4 | Mono | Off (0) | Off | Off | Off | On | On | yes |
| 5 | Enabled | On (1) | Off | Off | Off | On | On | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
