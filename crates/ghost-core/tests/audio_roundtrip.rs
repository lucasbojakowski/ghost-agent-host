use ghost_core::{analyze_audio, AnalysisConfig, AudioBuffer};

#[test]
fn analyses_stereo_sine() {
    let sample_rate = 48_000;
    let frames = sample_rate as usize;
    let channel: Vec<f32> = (0..frames)
        .map(|index| {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * index as f32 / sample_rate as f32;
            phase.sin() * 0.25
        })
        .collect();
    let audio = AudioBuffer {
        sample_rate,
        channels: vec![channel.clone(), channel],
    };
    let result = analyze_audio("sine", &audio, &AnalysisConfig::high()).unwrap();
    assert!((result.signal.spectrum.centroid_hz - 440.0).abs() < 100.0);
    assert!(result.signal.stereo.broadband_correlation > 0.99);
}
