use ghost_mix::{
    CompressorOperation, DynamicEqSettings, EqBandOperation, EqShape, ExpectedChange, MixOperation,
    MixPlan, PromptBundle,
};

use super::{AgentError, MixingAgent};
#[derive(Default)]
pub struct MockMixingAgent;

impl MixingAgent for MockMixingAgent {
    fn backend_name(&self) -> &'static str {
        "mock"
    }

    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError> {
        let analysis: ghost_core::AnalysisBundle =
            serde_json::from_str(&bundle.analysis_text_json)?;
        let signal = &analysis.signal;
        let mut operations = Vec::new();
        let mut expected_changes = Vec::new();
        let bands = &signal.spectrum.bands;

        if bands.low_mid_db > bands.mid_db + 4.0 {
            operations.push(MixOperation::EqBand {
                settings: EqBandOperation {
                    band_id: "agent-low-mid-control".into(),
                    enabled: true,
                    shape: EqShape::Bell,
                    frequency_hz: 260.0,
                    gain_db: -2.0,
                    q: 1.05,
                    slope_db_oct: None,
                    channel_mode: "stereo".into(),
                    dynamic: Some(DynamicEqSettings {
                        enabled: true,
                        range_db: -1.5,
                        threshold_db: None,
                    }),
                    rationale:
                        "Reduce persistent low-mid concentration without removing bass weight."
                            .into(),
                    evidence: vec![format!(
                        "low_mid_db={:.2}; mid_db={:.2}",
                        bands.low_mid_db, bands.mid_db
                    )],
                },
            });
            expected_changes.push(ExpectedChange {
                metric: "spectrum.bands.low_mid_db".into(),
                direction: "decrease".into(),
                maximum_delta: Some(4.0),
                unit: Some("dB".into()),
            });
        }

        if signal.loudness.crest_factor_db > 12.0 && signal.dynamics.transient_density_hz > 1.0 {
            operations.push(MixOperation::Compressor {
                settings: CompressorOperation {
                    enabled: true,
                    style: "clean".into(),
                    threshold_db: -18.0,
                    ratio: 2.0,
                    knee_db: 6.0,
                    attack_ms: 25.0,
                    release_ms: 140.0,
                    range_db: 3.0,
                    mix_percent: 70.0,
                    output_gain_db: 0.0,
                    rationale:
                        "Control event-to-event level variation while preserving initial attack."
                            .into(),
                    evidence: vec![format!(
                        "crest_factor_db={:.2}; transient_density_hz={:.2}",
                        signal.loudness.crest_factor_db, signal.dynamics.transient_density_hz
                    )],
                },
            });
            expected_changes.push(ExpectedChange {
                metric: "loudness.crest_factor_db".into(),
                direction: "decrease".into(),
                maximum_delta: Some(3.0),
                unit: Some("dB".into()),
            });
        }

        if let Some(resonance) = signal.spectrum.resonances.first() {
            if resonance.prominence_db > 7.0 {
                operations.push(MixOperation::EqBand {
                    settings: EqBandOperation {
                        band_id: "agent-resonance-control".into(),
                        enabled: true,
                        shape: EqShape::Bell,
                        frequency_hz: resonance.frequency_hz,
                        gain_db: -resonance.prominence_db.min(4.5) * 0.55,
                        q: (1.0 / resonance.bandwidth_octaves.max(0.08)).clamp(1.0, 12.0),
                        slope_db_oct: None,
                        channel_mode: "stereo".into(),
                        dynamic: Some(DynamicEqSettings {
                            enabled: true,
                            range_db: -2.0,
                            threshold_db: None,
                        }),
                        rationale:
                            "Control the most prominent persistent narrow-band concentration."
                                .into(),
                        evidence: vec![format!(
                            "resonance_hz={:.1}; prominence_db={:.2}",
                            resonance.frequency_hz, resonance.prominence_db
                        )],
                    },
                });
            }
        }

        Ok(MixPlan {
            schema_version: "ghost.mix-plan/1".into(),
            summary: if operations.is_empty() {
                "No conservative EQ or compression intervention was justified by the current text evidence."
                    .into()
            } else {
                "Conservative plugin-in-the-loop proposal derived from measured spectral and dynamic evidence."
                    .into()
            },
            confidence: if operations.is_empty() { 0.55 } else { 0.78 },
            assumptions: vec![
                "The captured region is representative of the requested source.".into(),
                "The mock backend approximates, but does not duplicate, FabFilter processing."
                    .into(),
            ],
            operations,
            expected_changes,
            cautions: vec!["Verify the result in context and with level-matched A/B.".into()],
        })
    }
}
