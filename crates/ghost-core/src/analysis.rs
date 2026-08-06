use std::cmp::Ordering;

use ebur128::{EbuR128, Mode};
use realfft::RealFftPlanner;
use thiserror::Error;
use uuid::Uuid;

use crate::audio::AudioBuffer;
use crate::model::{
    AnalysisBundle, AnalysisConfig, BandEnergy, CaptureMetadata, DynamicsFeatures,
    LoudnessFeatures, QualityFlag, ResonanceCandidate, SignalAnalysis, SignalIntegrity,
    SpectralFeatures, StereoFeatures,
};

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("invalid analysis configuration: {0}")]
    InvalidConfig(String),
    #[error("audio buffer is empty")]
    EmptyAudio,
    #[error("FFT failed: {0}")]
    Fft(String),
    #[error("loudness analysis failed: {0}")]
    Loudness(String),
}

#[derive(Default)]
struct SpectralAccumulator {
    centroid_sum: f64,
    rolloff_sum: f64,
    flatness_sum: f64,
    flux_sum: f64,
    frames: usize,
    frame_centroids: Vec<f32>,
    average_spectrum: Vec<f64>,
    average_spectrum_frames: usize,
    fft_size: usize,
}

pub fn analyze_audio(
    source_name: impl Into<String>,
    audio: &AudioBuffer,
    config: &AnalysisConfig,
) -> Result<AnalysisBundle, AnalysisError> {
    config.validate().map_err(AnalysisError::InvalidConfig)?;
    if audio.frames() == 0 || audio.channels.is_empty() {
        return Err(AnalysisError::EmptyAudio);
    }

    let mono = audio.mono_mix();
    let content_hash = hash_audio(audio);
    let mut integrity = analyze_integrity(audio);
    let (loudness, true_peak_dbtp) = analyze_loudness(audio, &mono)?;
    integrity.true_peak_dbtp = true_peak_dbtp;
    let spectrum = analyze_spectrum(&mono, audio.sample_rate, config)?;
    let dynamics = analyze_dynamics(&mono, audio.sample_rate, config.transient_sensitivity);
    let stereo = analyze_stereo(audio);
    let flags = quality_flags(audio, &integrity, &loudness);

    Ok(AnalysisBundle {
        schema_version: "ghost.analysis/1".into(),
        analyzer_version: env!("CARGO_PKG_VERSION").into(),
        configuration: config.clone(),
        capture: CaptureMetadata {
            capture_id: Uuid::new_v4(),
            source_name: source_name.into(),
            sample_rate: audio.sample_rate,
            channels: audio.channels.len(),
            frames: audio.frames(),
            duration_seconds: audio.duration_seconds(),
            content_hash,
            transport_bpm: None,
            transport_start_samples: None,
        },
        signal: SignalAnalysis {
            integrity,
            loudness,
            spectrum,
            dynamics,
            stereo,
            flags,
        },
    })
}

fn hash_audio(audio: &AudioBuffer) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&audio.sample_rate.to_le_bytes());
    hasher.update(&(audio.channels.len() as u64).to_le_bytes());
    for channel in &audio.channels {
        for sample in channel {
            hasher.update(&sample.to_le_bytes());
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn analyze_integrity(audio: &AudioBuffer) -> SignalIntegrity {
    let mut peak = 0.0_f64;
    let mut clipped = 0_u64;
    let mut non_finite = 0_u64;
    let mut silence = 0_u64;
    let silence_threshold = db_to_gain(-90.0);
    let mut dc = vec![0.0_f64; audio.channels.len()];
    let mut total = 0_u64;

    for (channel_index, channel) in audio.channels.iter().enumerate() {
        for &sample in channel {
            total += 1;
            if !sample.is_finite() {
                non_finite += 1;
                continue;
            }
            let value = sample as f64;
            dc[channel_index] += value;
            peak = peak.max(value.abs());
            if value.abs() >= 1.0 {
                clipped += 1;
            }
            if value.abs() <= silence_threshold {
                silence += 1;
            }
        }
        dc[channel_index] /= channel.len().max(1) as f64;
    }

    SignalIntegrity {
        sample_peak_dbfs: gain_to_db(peak),
        true_peak_dbtp: None,
        clipped_samples: clipped,
        dc_offset: dc,
        silence_ratio: silence as f64 / total.max(1) as f64,
        non_finite_samples: non_finite,
    }
}

fn analyze_loudness(
    audio: &AudioBuffer,
    mono: &[f32],
) -> Result<(LoudnessFeatures, Option<f64>), AnalysisError> {
    let channels = audio.channels.len() as u32;
    let mut meter = EbuR128::new(
        channels,
        audio.sample_rate,
        Mode::I | Mode::LRA | Mode::TRUE_PEAK,
    )
    .map_err(|error| AnalysisError::Loudness(error.to_string()))?;
    meter
        .add_frames_f32(&audio.interleaved())
        .map_err(|error| AnalysisError::Loudness(error.to_string()))?;

    let integrated_lufs = meter.loudness_global().ok().filter(|value| value.is_finite());
    let loudness_range_lu = meter.loudness_range().ok().filter(|value| value.is_finite());
    let mut true_peak = f64::NEG_INFINITY;
    for channel in 0..channels {
        if let Ok(value) = meter.true_peak(channel) {
            if value > 0.0 && value.is_finite() {
                true_peak = true_peak.max(gain_to_db(value));
            }
        }
    }

    let mean_square = mono
        .iter()
        .map(|sample| (*sample as f64).powi(2))
        .sum::<f64>()
        / mono.len().max(1) as f64;
    let rms = mean_square.sqrt();
    let peak = mono
        .iter()
        .map(|sample| sample.abs() as f64)
        .fold(0.0_f64, f64::max);
    let windows = windowed_rms_db(mono, audio.sample_rate, 0.4, 0.1);

    let features = LoudnessFeatures {
        integrated_lufs,
        loudness_range_lu,
        rms_dbfs: gain_to_db(rms),
        crest_factor_db: gain_to_db(peak) - gain_to_db(rms),
        short_term_proxy_dbfs_p10: percentile(&windows, 0.10),
        short_term_proxy_dbfs_p50: percentile(&windows, 0.50),
        short_term_proxy_dbfs_p90: percentile(&windows, 0.90),
    };

    Ok((features, true_peak.is_finite().then_some(true_peak)))
}

fn analyze_spectrum(
    mono: &[f32],
    sample_rate: u32,
    config: &AnalysisConfig,
) -> Result<SpectralFeatures, AnalysisError> {
    let mut combined = SpectralAccumulator::default();
    let mut best = SpectralAccumulator::default();

    for &fft_size in &config.fft_sizes {
        if mono.len() < fft_size {
            continue;
        }
        let current = spectral_pass(mono, sample_rate, fft_size, config.hop_ratio)?;
        if current.fft_size > best.fft_size {
            best = current;
        } else {
            combined.centroid_sum += current.centroid_sum;
            combined.rolloff_sum += current.rolloff_sum;
            combined.flatness_sum += current.flatness_sum;
            combined.flux_sum += current.flux_sum;
            combined.frames += current.frames;
            combined.frame_centroids.extend(current.frame_centroids);
        }
    }

    if best.frames == 0 {
        return Err(AnalysisError::Fft(
            "audio shorter than every configured FFT size".into(),
        ));
    }

    combined.centroid_sum += best.centroid_sum;
    combined.rolloff_sum += best.rolloff_sum;
    combined.flatness_sum += best.flatness_sum;
    combined.flux_sum += best.flux_sum;
    combined.frames += best.frames;
    combined.frame_centroids.extend(best.frame_centroids.clone());

    let frame_count = combined.frames.max(1) as f64;
    let spectrum = &best.average_spectrum;
    let bands = band_energies(spectrum, best.fft_size, sample_rate);
    let tilt = spectral_tilt(spectrum, best.fft_size, sample_rate);
    let resonances = resonance_candidates(
        spectrum,
        best.fft_size,
        sample_rate,
        config.resonance_threshold_db,
        config.minimum_frequency_hz as f64,
        config.maximum_frequency_hz as f64,
    );

    Ok(SpectralFeatures {
        centroid_hz: combined.centroid_sum / frame_count,
        rolloff_85_hz: combined.rolloff_sum / frame_count,
        flatness: combined.flatness_sum / frame_count,
        flux_mean: combined.flux_sum / frame_count,
        tilt_db_per_octave: tilt,
        bands,
        resonances,
        frame_centroid_hz: if config.retain_frame_series {
            best.frame_centroids
        } else {
            Vec::new()
        },
    })
}

fn spectral_pass(
    mono: &[f32],
    sample_rate: u32,
    fft_size: usize,
    hop_ratio: f32,
) -> Result<SpectralAccumulator, AnalysisError> {
    let hop = ((fft_size as f32 * hop_ratio).round() as usize).max(1);
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let window: Vec<f32> = (0..fft_size)
        .map(|index| {
            0.5 - 0.5
                * ((2.0 * std::f32::consts::PI * index as f32) / fft_size as f32).cos()
        })
        .collect();

    let mut accumulator = SpectralAccumulator {
        average_spectrum: vec![0.0; output.len()],
        fft_size,
        ..Default::default()
    };
    let mut previous = vec![0.0_f64; output.len()];

    for frame_start in (0..=mono.len() - fft_size).step_by(hop) {
        for index in 0..fft_size {
            input[index] = mono[frame_start + index] * window[index];
        }
        fft.process(&mut input, &mut output)
            .map_err(|error| AnalysisError::Fft(error.to_string()))?;

        let magnitudes: Vec<f64> = output
            .iter()
            .map(|bin| (bin.norm() as f64).max(1.0e-20))
            .collect();
        let total = magnitudes.iter().sum::<f64>().max(1.0e-20);
        let centroid = magnitudes
            .iter()
            .enumerate()
            .map(|(index, magnitude)| frequency(index, fft_size, sample_rate) * magnitude)
            .sum::<f64>()
            / total;

        let mut cumulative = 0.0;
        let target = total * 0.85;
        let mut rolloff = 0.0;
        for (index, magnitude) in magnitudes.iter().enumerate() {
            cumulative += magnitude;
            if cumulative >= target {
                rolloff = frequency(index, fft_size, sample_rate);
                break;
            }
        }

        let geometric = (magnitudes.iter().map(|value| value.ln()).sum::<f64>()
            / magnitudes.len() as f64)
            .exp();
        let arithmetic = total / magnitudes.len() as f64;
        let flatness = geometric / arithmetic.max(1.0e-20);
        let flux = magnitudes
            .iter()
            .zip(&previous)
            .map(|(current, prior)| (current - prior).max(0.0).powi(2))
            .sum::<f64>()
            .sqrt()
            / magnitudes.len() as f64;

        accumulator.centroid_sum += centroid;
        accumulator.rolloff_sum += rolloff;
        accumulator.flatness_sum += flatness;
        accumulator.flux_sum += flux;
        accumulator.frames += 1;
        accumulator.frame_centroids.push(centroid as f32);
        for (average, magnitude) in accumulator.average_spectrum.iter_mut().zip(&magnitudes) {
            *average += magnitude;
        }
        accumulator.average_spectrum_frames += 1;
        previous.copy_from_slice(&magnitudes);
    }

    let divisor = accumulator.average_spectrum_frames.max(1) as f64;
    for value in &mut accumulator.average_spectrum {
        *value /= divisor;
    }
    Ok(accumulator)
}

fn band_energies(spectrum: &[f64], fft_size: usize, sample_rate: u32) -> BandEnergy {
    let band = |low: f64, high: f64| -> f64 {
        let energy = spectrum
            .iter()
            .enumerate()
            .filter_map(|(index, magnitude)| {
                let hz = frequency(index, fft_size, sample_rate);
                (hz >= low && hz < high).then_some(magnitude.powi(2))
            })
            .sum::<f64>();
        10.0 * energy.max(1.0e-30).log10()
    };
    BandEnergy {
        sub_db: band(20.0, 60.0),
        bass_db: band(60.0, 150.0),
        low_mid_db: band(150.0, 500.0),
        mid_db: band(500.0, 2_000.0),
        high_mid_db: band(2_000.0, 5_000.0),
        presence_db: band(5_000.0, 10_000.0),
        air_db: band(10_000.0, 22_000.0),
    }
}

fn spectral_tilt(spectrum: &[f64], fft_size: usize, sample_rate: u32) -> f64 {
    let mut x = Vec::new();
    let mut y = Vec::new();
    for (index, magnitude) in spectrum.iter().enumerate().skip(1) {
        let hz = frequency(index, fft_size, sample_rate);
        if (40.0..=16_000.0).contains(&hz) {
            x.push((hz / 1_000.0).log2());
            y.push(20.0 * magnitude.max(1.0e-20).log10());
        }
    }
    linear_slope(&x, &y)
}

fn resonance_candidates(
    spectrum: &[f64],
    fft_size: usize,
    sample_rate: u32,
    threshold_db: f32,
    minimum_hz: f64,
    maximum_hz: f64,
) -> Vec<ResonanceCandidate> {
    let db: Vec<f64> = spectrum
        .iter()
        .map(|value| 20.0 * value.max(1.0e-20).log10())
        .collect();
    let radius = (fft_size / 256).clamp(4, 96);
    let mut candidates = Vec::new();

    for index in radius..db.len().saturating_sub(radius) {
        let hz = frequency(index, fft_size, sample_rate);
        if hz < minimum_hz || hz > maximum_hz {
            continue;
        }
        let neighborhood = &db[index - radius..=index + radius];
        let median = percentile(neighborhood, 0.5);
        let prominence = db[index] - median;
        if prominence >= threshold_db as f64
            && db[index] >= db[index - 1]
            && db[index] >= db[index + 1]
        {
            let bandwidth_hz = radius as f64 * sample_rate as f64 / fft_size as f64;
            let bandwidth_octaves = ((hz + bandwidth_hz) / (hz - bandwidth_hz).max(1.0))
                .log2()
                .abs();
            candidates.push(ResonanceCandidate {
                frequency_hz: hz,
                prominence_db: prominence,
                persistence: 1.0,
                bandwidth_octaves,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .prominence_db
            .partial_cmp(&left.prominence_db)
            .unwrap_or(Ordering::Equal)
    });
    candidates.truncate(16);
    candidates
}

fn analyze_dynamics(mono: &[f32], sample_rate: u32, sensitivity: f32) -> DynamicsFeatures {
    let attack_coeff = smoothing_coefficient(sample_rate, 0.005);
    let release_coeff = smoothing_coefficient(sample_rate, 0.060);
    let mut envelope = Vec::with_capacity(mono.len());
    let mut current = 0.0_f64;
    for &sample in mono {
        let target = sample.abs() as f64;
        let coeff = if target > current {
            attack_coeff
        } else {
            release_coeff
        };
        current = coeff * current + (1.0 - coeff) * target;
        envelope.push(current);
    }

    let derivative: Vec<f64> = envelope
        .windows(2)
        .map(|window| (window[1] - window[0]).max(0.0))
        .collect();
    let mean = derivative.iter().sum::<f64>() / derivative.len().max(1) as f64;
    let stddev = (derivative
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / derivative.len().max(1) as f64)
        .sqrt();
    let threshold = mean + sensitivity as f64 * stddev;
    let refractory = (sample_rate as f64 * 0.020) as usize;
    let mut transients = 0_usize;
    let mut last = 0_usize.wrapping_sub(refractory);
    let mut strengths = Vec::new();
    for (index, value) in derivative.iter().enumerate() {
        if *value >= threshold && index.saturating_sub(last) >= refractory {
            transients += 1;
            strengths.push(*value);
            last = index;
        }
    }

    let envelope_db: Vec<f64> = envelope.iter().map(|value| gain_to_db(*value)).collect();
    let p10 = percentile(&envelope_db, 0.10);
    let p50 = percentile(&envelope_db, 0.50);
    let p90 = percentile(&envelope_db, 0.90);
    let variability = p90 - p10;
    let sustained = envelope
        .iter()
        .filter(|value| **value >= db_to_gain(p50))
        .count() as f64
        / envelope.len().max(1) as f64;

    DynamicsFeatures {
        transient_density_hz: transients as f64 / (mono.len() as f64 / sample_rate as f64),
        envelope_variability_db: variability,
        peak_to_median_db: gain_to_db(envelope.iter().copied().fold(0.0_f64, f64::max)) - p50,
        attack_strength_p90: percentile(&strengths, 0.90),
        sustained_energy_ratio: sustained,
    }
}

fn analyze_stereo(audio: &AudioBuffer) -> StereoFeatures {
    if audio.channels.len() < 2 {
        return StereoFeatures {
            broadband_correlation: 1.0,
            mid_side_ratio_db: 200.0,
            left_right_balance_db: 0.0,
            low_band_correlation: 1.0,
            high_band_correlation: 1.0,
        };
    }
    let left = &audio.channels[0];
    let right = &audio.channels[1];
    let broadband = correlation(left, right);
    let left_rms = rms(left);
    let right_rms = rms(right);

    let mut mid_energy = 0.0_f64;
    let mut side_energy = 0.0_f64;
    for (&l, &r) in left.iter().zip(right) {
        let mid = (l as f64 + r as f64) * 0.5;
        let side = (l as f64 - r as f64) * 0.5;
        mid_energy += mid * mid;
        side_energy += side * side;
    }

    let left_low = one_pole_lowpass(left, audio.sample_rate, 200.0);
    let right_low = one_pole_lowpass(right, audio.sample_rate, 200.0);
    let left_high: Vec<f32> = left.iter().zip(&left_low).map(|(a, b)| a - b).collect();
    let right_high: Vec<f32> = right.iter().zip(&right_low).map(|(a, b)| a - b).collect();

    StereoFeatures {
        broadband_correlation: broadband,
        mid_side_ratio_db: 10.0 * (mid_energy / side_energy.max(1.0e-30)).log10(),
        left_right_balance_db: gain_to_db(left_rms) - gain_to_db(right_rms),
        low_band_correlation: correlation(&left_low, &right_low),
        high_band_correlation: correlation(&left_high, &right_high),
    }
}

fn quality_flags(
    audio: &AudioBuffer,
    integrity: &SignalIntegrity,
    loudness: &LoudnessFeatures,
) -> Vec<QualityFlag> {
    let mut flags = Vec::new();
    if audio.duration_seconds() < 6.0 {
        flags.push(QualityFlag {
            code: "short_capture".into(),
            severity: "warning".into(),
            message: "Capture is shorter than six seconds; macro and loudness estimates are limited."
                .into(),
        });
    }
    if integrity.clipped_samples > 0 {
        flags.push(QualityFlag {
            code: "clipping".into(),
            severity: "warning".into(),
            message: format!("Detected {} clipped samples.", integrity.clipped_samples),
        });
    }
    if integrity.non_finite_samples > 0 {
        flags.push(QualityFlag {
            code: "non_finite".into(),
            severity: "error".into(),
            message: "Non-finite PCM values were ignored.".into(),
        });
    }
    if integrity.silence_ratio > 0.80 {
        flags.push(QualityFlag {
            code: "mostly_silent".into(),
            severity: "warning".into(),
            message: "More than 80% of the capture is near digital silence.".into(),
        });
    }
    if loudness.integrated_lufs.is_none() {
        flags.push(QualityFlag {
            code: "loudness_unavailable".into(),
            severity: "info".into(),
            message: "Standards-based integrated loudness was unavailable for this capture."
                .into(),
        });
    }
    flags
}

fn frequency(index: usize, fft_size: usize, sample_rate: u32) -> f64 {
    index as f64 * sample_rate as f64 / fft_size as f64
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|sample| (*sample as f64).powi(2))
        .sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt()
}

fn correlation(left: &[f32], right: &[f32]) -> f64 {
    let mut dot = 0.0_f64;
    let mut left_energy = 0.0_f64;
    let mut right_energy = 0.0_f64;
    for (&l, &r) in left.iter().zip(right) {
        let l = l as f64;
        let r = r as f64;
        dot += l * r;
        left_energy += l * l;
        right_energy += r * r;
    }
    dot / (left_energy.sqrt() * right_energy.sqrt()).max(1.0e-30)
}

fn one_pole_lowpass(samples: &[f32], sample_rate: u32, cutoff_hz: f64) -> Vec<f32> {
    let alpha = (-2.0 * std::f64::consts::PI * cutoff_hz / sample_rate as f64).exp();
    let mut state = 0.0_f64;
    samples
        .iter()
        .map(|sample| {
            state = alpha * state + (1.0 - alpha) * *sample as f64;
            state as f32
        })
        .collect()
}

fn smoothing_coefficient(sample_rate: u32, seconds: f64) -> f64 {
    (-1.0 / (seconds * sample_rate as f64)).exp()
}

fn windowed_rms_db(samples: &[f32], sample_rate: u32, window_s: f64, hop_s: f64) -> Vec<f64> {
    let window = (sample_rate as f64 * window_s).round() as usize;
    let hop = (sample_rate as f64 * hop_s).round() as usize;
    if window == 0 || samples.len() < window {
        return vec![gain_to_db(rms(samples))];
    }
    (0..=samples.len() - window)
        .step_by(hop.max(1))
        .map(|start| gain_to_db(rms(&samples[start..start + window])))
        .collect()
}

fn percentile(values: &[f64], quantile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let position = quantile.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
}

fn linear_slope(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }
    let mean_x = x.iter().sum::<f64>() / x.len() as f64;
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    let numerator = x
        .iter()
        .zip(y)
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let denominator = x.iter().map(|x| (x - mean_x).powi(2)).sum::<f64>();
    numerator / denominator.max(1.0e-30)
}

fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.max(1.0e-20).log10()
}

fn db_to_gain(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}
