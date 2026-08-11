//! Deterministic feature extraction entrypoint.

mod config;
mod dynamics;
mod features;
mod spectrum;

pub use config::{AnalysisConfig, QualityProfile};
pub use features::*;

use dynamics::{analyze_dynamics, analyze_stereo, quality_flags};
use spectrum::analyze_spectrum;

use std::cmp::Ordering;

use ebur128::{EbuR128, Mode};
use thiserror::Error;
use uuid::Uuid;

use crate::audio::AudioBuffer;
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

    let integrated_lufs = meter
        .loudness_global()
        .ok()
        .filter(|value| value.is_finite());
    let loudness_range_lu = meter
        .loudness_range()
        .ok()
        .filter(|value| value.is_finite());
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

fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.max(1.0e-20).log10()
}

fn db_to_gain(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}
