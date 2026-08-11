use ghost_audio::{analyze_audio, read_wav, write_wav_f32, AnalysisConfig, AudioBuffer};

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

#[test]
fn wav_roundtrip_is_self_contained() {
    let path = std::env::temp_dir().join(format!(
        "ghost-audio-roundtrip-{}-{}.wav",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let audio = AudioBuffer {
        sample_rate: 48_000,
        channels: vec![vec![0.0, 0.25, -0.25, 0.0], vec![0.0, -0.25, 0.25, 0.0]],
    };
    write_wav_f32(&path, &audio).unwrap();
    let decoded = read_wav(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(decoded.sample_rate, audio.sample_rate);
    assert_eq!(decoded.channels, audio.channels);
}
