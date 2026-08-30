# Fruity Chorus

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **12**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Delay | 15.02500 ms (0.5) | 0.05000 ms | 7.53750 ms | 15.02500 ms | 22.51250 ms | 30.00000 ms | yes |
| 2 | Depth | 2.25000 ms (0.45) | 0.00000 ms | 1.25000 ms | 2.50000 ms | 3.75000 ms | 5.00000 ms | yes |
| 3 | Stereo | 59 degrees (0.333) | 0 degrees | 45 degrees | 90 degrees | 135 degrees | 180 degrees | yes |
| 4 | LFO 1 Frequency | 0.45000 Hz (0.3) | 0.00000 Hz | 0.31250 Hz | 1.25000 Hz | 2.81250 Hz | 5.00000 Hz | yes |
| 5 | LFO 2 Frequency | 1.25000 Hz (0.5) | 0.00000 Hz | 0.31250 Hz | 1.25000 Hz | 2.81250 Hz | 5.00000 Hz | yes |
| 6 | LFO 3 Frequency | 2.45000 Hz (0.7) | 0.00000 Hz | 0.31250 Hz | 1.25000 Hz | 2.81250 Hz | 5.00000 Hz | yes |
| 7 | LFO 1 Wave | sin (0.0225) | sin | sin | sin^3 | multi-sin | multi-sin^3 | yes |
| 8 | LFO 2 Wave | sin (0) | sin | sin | sin^3 | multi-sin | multi-sin^3 | yes |
| 9 | LFO 3 Wave | sin (0) | sin | sin | sin^3 | multi-sin | multi-sin^3 | yes |
| 10 | Cross Type | Process HF (0) | Process HF | Process HF | Process LF | Process LF | Process LF | yes |
| 11 | Cross Cutoff | 320.24371 Hz (0.5) | 8.17580 Hz | 51.16882 Hz | 320.24371 Hz | 2004.26794 Hz | 12543.85352 Hz | yes |
| 12 | Wet only | no (0) | no | no | yes | yes | yes | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
