use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
        if self.fft_sizes.is_empty() || self.fft_sizes.iter().any(|size| !size.is_power_of_two()) {
            return Err("fft_sizes must be non-empty powers of two".into());
        }
        if !(0.03125..=1.0).contains(&self.hop_ratio) {
            return Err("hop_ratio must be between 0.03125 and 1.0".into());
        }
        if self.minimum_frequency_hz < 0.0 || self.maximum_frequency_hz <= self.minimum_frequency_hz
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
