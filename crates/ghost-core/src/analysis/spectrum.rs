use std::cmp::Ordering;

use realfft::RealFftPlanner;

use super::{
    percentile, AnalysisConfig, AnalysisError, BandEnergy, ResonanceCandidate, SpectralFeatures,
};
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
        .extend(best.frame_centroids.clone());

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
