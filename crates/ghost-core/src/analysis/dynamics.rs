use crate::audio::AudioBuffer;

use super::{
    correlation, db_to_gain, gain_to_db, one_pole_lowpass, percentile, rms, smoothing_coefficient,
    DynamicsFeatures, LoudnessFeatures, QualityFlag, SignalIntegrity, StereoFeatures,
};
pub(super) fn analyze_dynamics(
    mono: &[f32],
    sample_rate: u32,
    sensitivity: f32,
) -> DynamicsFeatures {
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

pub(super) fn analyze_stereo(audio: &AudioBuffer) -> StereoFeatures {
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

pub(super) fn quality_flags(
    audio: &AudioBuffer,
    integrity: &SignalIntegrity,
    loudness: &LoudnessFeatures,
) -> Vec<QualityFlag> {
    let mut flags = Vec::new();
    if audio.duration_seconds() < 6.0 {
        flags.push(QualityFlag {
            code: "short_capture".into(),
            severity: "warning".into(),
            message:
                "Capture is shorter than six seconds; macro and loudness estimates are limited."
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
            message: "Standards-based integrated loudness was unavailable for this capture.".into(),
        });
    }
    flags
}
