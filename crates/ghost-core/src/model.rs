use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityProfile {
    Live,
    High,
    Maximum,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisConfig {
    pub profile: QualityProfile,
    pub fft_sizes: Vec<usize>,
    pub hop_ratio: f32,
    pub minimum_frequency_hz: f32,
    pub maximum_frequency_hz: f32,
    pub resonance_threshold_db: f32,
    pub transient_sensitivity: f32,
    pub true_peak_oversample: u32,
    pub retain_frame_series: bool,
}

impl AnalysisConfig {
    pub fn live() -> Self {
        Self {
            profile: QualityProfile::Live,
            fft_sizes: vec![1024, 4096],
            hop_ratio: 0.5,
            minimum_frequency_hz: 30.0,
            maximum_frequency_hz: 20_000.0,
            resonance_threshold_db: 7.0,
            transient_sensitivity: 3.2,
            true_peak_oversample: 2,
            retain_frame_series: false,
        }
    }

    pub fn high() -> Self {
        Self {
            profile: QualityProfile::High,
            fft_sizes: vec![2048, 8192, 16384],
            hop_ratio: 0.25,
            minimum_frequency_hz: 20.0,
            maximum_frequency_hz: 22_000.0,
            resonance_threshold_db: 5.5,
            transient_sensitivity: 2.7,
            true_peak_oversample: 4,
            retain_frame_series: true,
        }
    }

    pub fn maximum() -> Self {
        Self {
            profile: QualityProfile::Maximum,
            fft_sizes: vec![2048, 8192, 32768],
            hop_ratio: 0.125,
            minimum_frequency_hz: 10.0,
            maximum_frequency_hz: 24_000.0,
            resonance_threshold_db: 4.5,
            transient_sensitivity: 2.3,
            true_peak_oversample: 8,
            retain_frame_series: true,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.fft_sizes.is_empty() || self.fft_sizes.iter().any(|n| !n.is_power_of_two()) {
            return Err("fft_sizes must be non-empty powers of two".into());
        }
        if !(0.03125..=1.0).contains(&self.hop_ratio) {
            return Err("hop_ratio must be between 0.03125 and 1.0".into());
        }
        if self.minimum_frequency_hz < 0.0
            || self.maximum_frequency_hz <= self.minimum_frequency_hz
        {
            return Err("invalid frequency range".into());
        }
        Ok(())
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self::maximum()
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StructuredIntent {
    pub source: String,
    pub role: String,
    pub style: String,
    pub goal: String,
    pub problem: Option<String>,
    pub intensity: String,
    pub preserve: Vec<String>,
    pub scope: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum UserIntent {
    Freeform { prompt: String },
    Structured { context: StructuredIntent },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginTarget {
    ProQ4,
    ProC3,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EqShape {
    Bell,
    LowShelf,
    HighShelf,
    LowCut,
    HighCut,
    Notch,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicEqSettings {
    pub enabled: bool,
    pub range_db: f64,
    pub threshold_db: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EqBandOperation {
    pub band_id: String,
    pub enabled: bool,
    pub shape: EqShape,
    pub frequency_hz: f64,
    pub gain_db: f64,
    pub q: f64,
    pub slope_db_oct: Option<f64>,
    pub channel_mode: String,
    pub dynamic: Option<DynamicEqSettings>,
    pub rationale: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompressorOperation {
    pub enabled: bool,
    pub style: String,
    pub threshold_db: f64,
    pub ratio: f64,
    pub knee_db: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub range_db: f64,
    pub mix_percent: f64,
    pub output_gain_db: f64,
    pub rationale: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MixOperation {
    EqBand { settings: EqBandOperation },
    Compressor { settings: CompressorOperation },
    Bypass { target: PluginTarget, bypassed: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExpectedChange {
    pub metric: String,
    pub direction: String,
    pub maximum_delta: Option<f64>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixPlan {
    pub schema_version: String,
    pub summary: String,
    pub confidence: f64,
    pub assumptions: Vec<String>,
    pub operations: Vec<MixOperation>,
    pub expected_changes: Vec<ExpectedChange>,
    pub cautions: Vec<String>,
}
