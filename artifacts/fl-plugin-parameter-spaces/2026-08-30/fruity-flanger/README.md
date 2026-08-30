# Fruity Flanger

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **12**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Delay | 0.00000 ms (0) | 0.00000 ms | 1.25000 ms | 3.33333 ms | 7.50000 ms | 20.00000 ms | yes |
| 2 | Depth | 2.35294 ms (0.4) | 0.00000 ms | 1.25000 ms | 3.33333 ms | 7.50000 ms | 20.00000 ms | yes |
| 3 | Rate | 0.31250 Hz (0.4) | 0.00000 Hz | 0.16129 Hz | 0.45455 Hz | 1.15385 Hz | 5.00000 Hz | yes |
| 4 | Phase | 118 degrees (0.3278) | 0 degrees | 90 degrees | 180 degrees | 270 degrees | 360 degrees | yes |
| 5 | Damp | 0.00000 (0) | 0.00000 | 0.25000 | 0.50000 | 0.75000 | 1.00000 | yes |
| 6 | Shape | s\|-------------t (0) | s\|-------------t | s---\|----------t | s------\|-------t | s----------\|---t | s-------------\|t | yes |
| 7 | Feed | 23 percent (0.23) | 0 percent | 25 percent | 50 percent | 75 percent | 100 percent | yes |
| 8 | Invert feedback | on (1) | off | off | off | on | on | yes |
| 9 | Invert wet | on (1) | off | off | off | on | on | yes |
| 10 | Dry | -3.09804 dB (0.7) |    -oo   dB | -12.04120 dB | -6.02060 dB | -2.49877 dB | 0.00000 dB | yes |
| 11 | Wet | -4.43698 dB (0.6) |    -oo   dB | -12.04120 dB | -6.02060 dB | -2.49877 dB | 0.00000 dB | yes |
| 12 | Cross | -6.02060 dB (0.5) |    -oo   dB | -12.04120 dB | -6.02060 dB | -2.49877 dB | 0.00000 dB | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
