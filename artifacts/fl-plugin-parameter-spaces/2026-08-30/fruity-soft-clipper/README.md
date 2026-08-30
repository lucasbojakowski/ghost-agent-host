# Fruity Soft Clipper

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **2**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Threshold | -4.4dB (0.7857) | -51.8dB | -19.2dB | -11.1dB | -5.2dB | 0.0dB | yes |
| 2 | Post gain | 80% (0.8) | 0% | 25% | 50% | 75% | 100% | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
