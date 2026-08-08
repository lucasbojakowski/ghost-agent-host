use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AnalysisConfig;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaptureMetadata {
    pub capture_id: Uuid,
    pub source_name: String,
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: usize,
    pub duration_seconds: f64,
    pub content_hash: String,
    pub transport_bpm: Option<f64>,
    pub transport_start_samples: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SignalIntegrity {
    pub sample_peak_dbfs: f64,
    pub true_peak_dbtp: Option<f64>,
    pub clipped_samples: u64,
    pub dc_offset: Vec<f64>,
    pub silence_ratio: f64,
    pub non_finite_samples: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LoudnessFeatures {
    pub integrated_lufs: Option<f64>,
    pub loudness_range_lu: Option<f64>,
    pub rms_dbfs: f64,
    pub crest_factor_db: f64,
    pub short_term_proxy_dbfs_p10: f64,
    pub short_term_proxy_dbfs_p50: f64,
    pub short_term_proxy_dbfs_p90: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct BandEnergy {
    pub sub_db: f64,
    pub bass_db: f64,
    pub low_mid_db: f64,
    pub mid_db: f64,
    pub high_mid_db: f64,
    pub presence_db: f64,
    pub air_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResonanceCandidate {
    pub frequency_hz: f64,
    pub prominence_db: f64,
    pub persistence: f64,
    pub bandwidth_octaves: f64,
}

/// Compact, display-oriented spectrum sample. Magnitude is normalized so the strongest retained
/// spectral point is 0 dB and the floor is limited to -96 dB. This keeps rendering data useful
/// without exposing every FFT bin in persisted analysis/context payloads.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SpectrumPoint {
    pub frequency_hz: f32,
    pub magnitude_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SpectralFeatures {
    pub centroid_hz: f64,
    pub rolloff_85_hz: f64,
    pub flatness: f64,
    pub flux_mean: f64,
    pub tilt_db_per_octave: f64,
    pub bands: BandEnergy,
    pub resonances: Vec<ResonanceCandidate>,
    pub frame_centroid_hz: Vec<f32>,
    #[serde(default)]
    pub display_spectrum: Vec<SpectrumPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DynamicsFeatures {
    pub transient_density_hz: f64,
    pub envelope_variability_db: f64,
    pub peak_to_median_db: f64,
    pub attack_strength_p90: f64,
    pub sustained_energy_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct StereoFeatures {
    pub broadband_correlation: f64,
    pub mid_side_ratio_db: f64,
    pub left_right_balance_db: f64,
    pub low_band_correlation: f64,
    pub high_band_correlation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QualityFlag {
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignalAnalysis {
    pub integrity: SignalIntegrity,
    pub loudness: LoudnessFeatures,
    pub spectrum: SpectralFeatures,
    pub dynamics: DynamicsFeatures,
    pub stereo: StereoFeatures,
    pub flags: Vec<QualityFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisBundle {
    pub schema_version: String,
    pub analyzer_version: String,
    pub configuration: AnalysisConfig,
    pub capture: CaptureMetadata,
    pub signal: SignalAnalysis,
}
