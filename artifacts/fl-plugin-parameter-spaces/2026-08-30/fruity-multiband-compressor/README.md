# Fruity Multiband Compressor

This is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.

Parameters: **28**. Validation: **passed**.

| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |
|---:|---|---|---|---|---|---|---|:---:|
| 1 | Master volume | 0.0dB (0.5) | -oo | -7.7dB | 0.0dB | 5.2dB | 9.5dB | yes |
| 2 | Limiter | OFF (0) | OFF | OFF | OFF | ON | ON | yes |
| 3 | Filter type | BW IIR (0) | BW IIR | BW IIR | BW IIR | LP FIR | LP FIR | yes |
| 4 | High band state | ON (0) | ON | ON | muted | bypassed | bypassed | yes |
| 5 | H Freq | 9237Hz (0.8498) | 20Hz | 160Hz | 1118Hz | 5321Hz | 20000Hz | yes |
| 6 | H Output | 0.0dB (0.3333) | -oo | -3.3dB | 5.2dB | 11.5dB | 16.9dB | yes |
| 7 | H Threshold | -18.0dB (0.7) | -60.0dB | -45.0dB | -30.0dB | -15.0dB | 0.0dB | yes |
| 8 | H Ratio | 2.0:1 (0.5) | 1.0:1 | 1.3:1 | 2.0:1 | 4.0:1 | oo:1 | yes |
| 9 | H Attack | 10.0ms (0.0991) | 0.1ms | 25.1ms | 50.1ms | 75.0ms | 100.0ms | yes |
| 10 | H Release | 100.0ms (0.0909) | 10.0ms | 257.5ms | 505.0ms | 752.5ms | 1000.0ms | yes |
| 11 | H Knee | 80% (0.8) | 0% | 25% | 50% | 75% | 100% | yes |
| 12 | Mid band state | ON (0) | ON | ON | muted | bypassed | bypassed | yes |
| 13 | M Freq H | 9237Hz (0.8498) | 20Hz | 160Hz | 1118Hz | 5321Hz | 20000Hz | yes |
| 14 | M Freq L | 300Hz (0.3243) | 20Hz | 160Hz | 1118Hz | 5321Hz | 20000Hz | yes |
| 15 | M Output | 0.0dB (0.3333) | -oo | -3.3dB | 5.2dB | 11.5dB | 16.9dB | yes |
| 16 | M Threshold | -15.3dB (0.7458) | -60.0dB | -45.0dB | -30.0dB | -15.0dB | 0.0dB | yes |
| 17 | M Ratio | 2.0:1 (0.5) | 1.0:1 | 1.3:1 | 2.0:1 | 4.0:1 | oo:1 | yes |
| 18 | M Attack | 10.0ms (0.0991) | 0.1ms | 25.1ms | 50.1ms | 75.0ms | 100.0ms | yes |
| 19 | M Release | 100.0ms (0.0909) | 10.0ms | 257.5ms | 505.0ms | 752.5ms | 1000.0ms | yes |
| 20 | M Knee | 80% (0.8) | 0% | 25% | 50% | 75% | 100% | yes |
| 21 | Low band state | ON (0) | ON | ON | muted | bypassed | bypassed | yes |
| 22 | L Freq | 300Hz (0.3243) | 20Hz | 160Hz | 1118Hz | 5321Hz | 20000Hz | yes |
| 23 | L Output | 0.0dB (0.3333) | -oo | -3.3dB | 5.2dB | 11.5dB | 16.9dB | yes |
| 24 | L Threshold | -18.0dB (0.7) | -60.0dB | -45.0dB | -30.0dB | -15.0dB | 0.0dB | yes |
| 25 | L Ratio | 2.0:1 (0.5) | 1.0:1 | 1.3:1 | 2.0:1 | 4.0:1 | oo:1 | yes |
| 26 | L Attack | 10.0ms (0.0991) | 0.1ms | 25.1ms | 50.1ms | 75.0ms | 100.0ms | yes |
| 27 | L Release | 100.0ms (0.0909) | 10.0ms | 257.5ms | 505.0ms | 752.5ms | 1000.0ms | yes |
| 28 | L Knee | 80% (0.8) | 0% | 25% | 50% | 75% | 100% | yes |

Caveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.
