# Fruity Phaser

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **9**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Sweep frequency | 0.50000 Hz (0.5) | 0.00000 Hz | 0.25000 Hz | 0.50000 Hz | 0.75000 Hz | 1.00000 Hz | yes |
| 2 | Minimum depth | 0.10000 (0.1) | 0.00000 | 0.25000 | 0.50000 | 0.75000 | 1.00000 | yes |
| 3 | Maximum depth | 0.80000 (0.8) | 0.00000 | 0.25000 | 0.50000 | 0.75000 | 1.00000 | yes |
| 4 | Frequency range | small (0) | small | small | small | large | large | yes |
| 5 | Stereo | 0.50000 phase (0.5) | 0.00000 phase | 0.25000 phase | 0.50000 phase | 0.75000 phase | 1.00000 phase | yes |
| 6 | Number of stages | 8 (0.3182) | 1 | 7 | 12 | 17 | 23 | yes |
| 7 | Feedback | 0.40000 amount (0.4) | 0.00000 amount | 0.25000 amount | 0.50000 amount | 0.75000 amount | 1.00000 amount | yes |
| 8 | Dry - Wet | 50 % wet (0.5) | 100 % wet | 75 % wet | 50 % wet | 25 % wet | 0 % wet | yes |
| 9 | Output gain | 4.08240 dB (0.8) | -oo   dB | -6.02060 dB | 0.00000 dB | 3.52183 dB | 6.02060 dB | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
