use ghost_mix::{EqBandOperation, EqShape, MixOperation, MixPlan, PromptBundle};

use super::{AgentError, MixingAgent};

#[derive(Default)]
pub struct MockMixingAgent;

impl MixingAgent for MockMixingAgent {
    fn backend_name(&self) -> &'static str {
        "mock"
    }

    fn propose(&mut self, _bundle: &PromptBundle) -> Result<MixPlan, AgentError> {
        Ok(validation_plan())
    }
}

fn validation_plan() -> MixPlan {
    // This backend is intentionally not an analysis simulation. It is a deterministic child-host
    // fixture used to prove that a semantic proposal can compile into public child parameters and
    // be applied through the CLAP host.
    //
    // Pro-Q 4 currently exposes its frequency parameter plain-value domain as roughly
    // 3.322..14.873 even though the semantic field is named `frequency_hz`. Until the semantic
    // mapper converts display Hz into the plugin's plain domain, keep these fixture frequencies in
    // the overlap between the workflow's accepted semantic range (>= 10) and Pro-Q's current plain
    // range. The resulting child bands are intentionally just a visible parameter-editing test.
    let bands = [
        ("mock-band-1", 10.0, -3.0, 0.75),
        ("mock-band-2", 12.0, 2.5, 1.10),
        ("mock-band-3", 14.0, -1.5, 2.00),
    ];

    let operations = bands
        .into_iter()
        .map(|(band_id, frequency_hz, gain_db, q)| MixOperation::EqBand {
            settings: EqBandOperation {
                band_id: band_id.into(),
                enabled: true,
                shape: EqShape::Bell,
                frequency_hz,
                gain_db,
                q,
                slope_db_oct: None,
                channel_mode: "stereo".into(),
                dynamic: None,
                rationale: "Deterministic mock band for child-parameter integration testing."
                    .into(),
                evidence: Vec::new(),
            },
        })
        .collect();

    MixPlan {
        schema_version: MixPlan::SCHEMA.into(),
        summary: "Deterministic three-band EQ fixture for validating child parameter writes."
            .into(),
        confidence: 1.0,
        assumptions: vec![
            "Mock mode intentionally ignores the prompt and analysis payload.".into(),
            "The proposal exists only to validate child-plugin parameter interaction.".into(),
        ],
        operations,
        expected_changes: Vec::new(),
        cautions: vec!["Validation fixture only; do not treat this as a mixing recommendation.".into()],
    }
}

#[cfg(test)]
mod tests {
    use ghost_mix::validate_mix_plan;

    use super::*;

    #[test]
    fn validation_plan_is_static_eq_only_and_schema_valid() {
        let plan = validation_plan();
        validate_mix_plan(&plan).unwrap();
        assert_eq!(plan.operations.len(), 3);

        for operation in plan.operations {
            let MixOperation::EqBand { settings } = operation else {
                panic!("mock validation fixture must contain EQ operations only");
            };
            assert!(settings.enabled);
            assert!(settings.dynamic.is_none());
            assert_eq!(settings.channel_mode, "stereo");
            assert_eq!(settings.shape, EqShape::Bell);
        }
    }
}
