# Fruity Reeverb 2

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **15**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Low cut | 75Hz (0.0188) | Off | 764Hz | 1509Hz | 2255Hz | 3000Hz | yes |
| 2 | High cut | 4.0kHz (0.162) | 0.5kHz | 5.9kHz | 11.3kHz | 16.7kHz | Off | yes |
| 3 | Predelay | 0ms (0) | 0ms | 250ms | 500ms | 750ms | 1000ms | yes |
| 4 | Room size | 50 (0.4949) | 1 | 26 | 51 | 75 | 100 | yes |
| 5 | Diffusion | 100 (1) | 0 | 25 | 50 | 75 | 100 | yes |
| 6 | Decay time | 1.5sec (0.0704) | 0.1sec | 5.1sec | 10.1sec | 15.0sec | 20.0sec | yes |
| 7 | High damping | 4.0kHz (0.162) | 0.5kHz | 5.9kHz | 11.3kHz | 16.7kHz | Off | yes |
| 8 | Bass multiplier | 100% (0.2857) | 20% | 90% | 160% | 230% | 300% | yes |
| 9 | Crossover | 500Hz (0.2405) | 25Hz | 519Hz | 1013Hz | 1506Hz | 2000Hz | yes |
| 10 | Stereo separation | Original (0.5) | 100% separated | 50% separated | Original | 50% merged | 100% merged | yes |
| 11 | Dry level | 100% (0.8) | 0% | 31% | 63% | 94% | 125% | yes |
| 12 | Early reflection level | 50% (0.4) | 0% | 31% | 63% | 94% | 125% | yes |
| 13 | Wet level | 50% (0.4) | 0% | 31% | 63% | 94% | 125% | yes |
| 14 | Mod Speed | 33% (0.334) | 0% | 25% | 50% | 75% | 100% | yes |
| 15 | Mod Depth | 0% (0) | 0% | 25% | 50% | 75% | 100% | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
