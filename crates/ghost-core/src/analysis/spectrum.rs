use std::cmp::Ordering;

use realfft::RealFftPlanner;

use super::{
    percentile, AnalysisConfig, AnalysisError, BandEnergy, ResonanceCandidate, SpectralFeatures,
    SpectrumPoint,
};

const DISPLAY_SPECTRUM_POINTS: usize = 320;
const DISPLAY_SPECTRUM_FLOOR_DB: f64 = -96.0;

#[derive(Default)]
struct SpectralAccumulator {
    centroid_sum: f64,
    rolloff_sum: f64,
    flatness_sum: f64,
    flux_sum: f64,
    frames: usize,
    frame_centroids: Vec<f32>,
    average_spectrum: Vec<f64>,
    frame_spectra_db: Vec<Vec<f32>>,
    average_spectrum_frames: usize,
    fft_size: usize,
}

pub(super) fn analyze_spectrum(
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
            if best.frames > 0 {
                combined.centroid_sum += best.centroid_sum;
                combined.rolloff_sum += best.rolloff_sum;
                combined.flatness_sum += best.flatness_sum;
                combined.flux_sum += best.flux_sum;
                combined.frames += best.frames;
                combined.frame_centroids.extend(best.frame_centroids);
            }
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
    combined
        .frame_centroids
        .extend(best.frame_centroids.iter().copied());

    let frame_count = combined.frames.max(1) as f64;
    let spectrum = &best.average_spectrum;
    let bands = band_energies(spectrum, best.fft_size, sample_rate);
    let tilt = spectral_tilt(spectrum, best.fft_size, sample_rate);
    let resonances = resonance_candidates(
        spectrum,
        &best.frame_spectra_db,
        best.fft_size,
        sample_rate,
        config.resonance_threshold_db,
        config.minimum_frequency_hz as f64,
        config.maximum_frequency_hz as f64,
    );
    let display_spectrum = display_spectrum(
        spectrum,
        best.fft_size,
        sample_rate,
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
        display_spectrum,
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
            0.5 - 0.5 * ((2.0 * std::f32::consts::PI * index as f32) / fft_size as f32).cos()
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
        accumulator.frame_spectra_db.push(
            magnitudes
                .iter()
                .map(|magnitude| (20.0 * magnitude.max(1.0e-20).log10()) as f32)
                .collect(),
        );
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
    frame_spectra_db: &[Vec<f32>],
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
            let persistent_frames = frame_spectra_db
                .iter()
                .filter(|frame| {
                    if frame.len() <= index + radius {
                        return false;
                    }
                    let local: Vec<f64> = frame[index - radius..=index + radius]
                        .iter()
                        .map(|value| f64::from(*value))
                        .collect();
                    let local_median = percentile(&local, 0.5);
                    f64::from(frame[index]) - local_median >= f64::from(threshold_db)
                })
                .count();
            let persistence = if frame_spectra_db.is_empty() {
                0.0
            } else {
                persistent_frames as f64 / frame_spectra_db.len() as f64
            };
            candidates.push(ResonanceCandidate {
                frequency_hz: hz,
                prominence_db: prominence,
                persistence,
                bandwidth_octaves,
            });
        }
    }

    candidates.sort_by(|left, right| {
        resonance_score(right)
            .partial_cmp(&resonance_score(left))
            .unwrap_or(Ordering::Equal)
    });
    candidates.truncate(16);
    candidates
}

fn resonance_score(candidate: &ResonanceCandidate) -> f64 {
    candidate.prominence_db * (0.35 + 0.65 * candidate.persistence)
}

fn display_spectrum(
    spectrum: &[f64],
    fft_size: usize,
    sample_rate: u32,
    minimum_hz: f64,
    maximum_hz: f64,
) -> Vec<SpectrumPoint> {
    if spectrum.len() < 2 || sample_rate == 0 || fft_size == 0 {
        return Vec::new();
    }
    let nyquist = f64::from(sample_rate) * 0.5;
    let minimum_hz = minimum_hz.max(20.0).min(nyquist);
    let maximum_hz = maximum_hz.min(nyquist).max(minimum_hz);
    let minimum_log = minimum_hz.ln();
    let maximum_log = maximum_hz.ln();

    let mut samples = Vec::with_capacity(DISPLAY_SPECTRUM_POINTS);
    let mut peak_db = f64::NEG_INFINITY;
    for index in 0..DISPLAY_SPECTRUM_POINTS {
        let t = index as f64 / (DISPLAY_SPECTRUM_POINTS - 1) as f64;
        let hz = (minimum_log + (maximum_log - minimum_log) * t).exp();
        let bin = hz * fft_size as f64 / f64::from(sample_rate);
        let lower = (bin.floor() as usize).min(spectrum.len() - 1);
        let upper = (lower + 1).min(spectrum.len() - 1);
        let fraction = (bin - lower as f64).clamp(0.0, 1.0);
        let magnitude = spectrum[lower] * (1.0 - fraction) + spectrum[upper] * fraction;
        let db = 20.0 * magnitude.max(1.0e-20).log10();
        peak_db = peak_db.max(db);
        samples.push((hz, db));
    }

    samples
        .into_iter()
        .map(|(frequency_hz, db)| SpectrumPoint {
            frequency_hz: frequency_hz as f32,
            magnitude_db: (db - peak_db).clamp(DISPLAY_SPECTRUM_FLOOR_DB, 0.0) as f32,
        })
        .collect()
}

fn frequency(index: usize, fft_size: usize, sample_rate: u32) -> f64 {
    index as f64 * sample_rate as f64 / fft_size as f64
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_spectrum_is_log_spaced_and_peak_normalized() {
        let spectrum = vec![1.0; 2049];
        let points = display_spectrum(&spectrum, 4096, 48_000, 20.0, 20_000.0);
        assert_eq!(points.len(), DISPLAY_SPECTRUM_POINTS);
        assert!(points.windows(2).all(|window| {
            window[1].frequency_hz > window[0].frequency_hz && window[1].magnitude_db <= 0.0
        }));
        assert!(points.iter().all(|point| point.magnitude_db.abs() < 1.0e-4));
    }
}
