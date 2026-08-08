use std::path::{Path, PathBuf};

use ghost_core::{analyze_audio, read_wav, AnalysisConfig};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

#[test]
fn maximum_profile_retains_frame_evidence() {
    let audio = read_wav(fixture("clean_reference.wav")).unwrap();
    let maximum = analyze_audio("clean", &audio, &AnalysisConfig::maximum()).unwrap();
    let live = analyze_audio("clean", &audio, &AnalysisConfig::live()).unwrap();
    assert!(!maximum.signal.spectrum.frame_centroid_hz.is_empty());
    assert!(live.signal.spectrum.frame_centroid_hz.is_empty());
}

#[test]
fn fixture_low_mid_excess_is_detected() {
    let clean_audio = read_wav(fixture("clean_reference.wav")).unwrap();
    let muddy_audio = read_wav(fixture("muddy_bass.wav")).unwrap();
    let config = AnalysisConfig::high();
    let clean = analyze_audio("clean", &clean_audio, &config).unwrap();
    let muddy = analyze_audio("muddy", &muddy_audio, &config).unwrap();
    assert!(muddy.signal.spectrum.bands.low_mid_db > clean.signal.spectrum.bands.low_mid_db + 1.0);
}

#[test]
fn fixture_phase_instability_is_detected() {
    let clean_audio = read_wav(fixture("clean_reference.wav")).unwrap();
    let phasey_audio = read_wav(fixture("phasey_wide.wav")).unwrap();
    let config = AnalysisConfig::high();
    let clean = analyze_audio("clean", &clean_audio, &config).unwrap();
    let phasey = analyze_audio("phasey", &phasey_audio, &config).unwrap();
    assert!(
        phasey.signal.stereo.high_band_correlation
            < clean.signal.stereo.high_band_correlation - 0.1
    );
}

#[test]
fn fixture_crushing_reduces_crest() {
    let clean_audio = read_wav(fixture("clean_reference.wav")).unwrap();
    let crushed_audio = read_wav(fixture("crushed_drums.wav")).unwrap();
    let config = AnalysisConfig::high();
    let clean = analyze_audio("clean", &clean_audio, &config).unwrap();
    let crushed = analyze_audio("crushed", &crushed_audio, &config).unwrap();
    assert!(crushed.signal.loudness.crest_factor_db < clean.signal.loudness.crest_factor_db);
}
