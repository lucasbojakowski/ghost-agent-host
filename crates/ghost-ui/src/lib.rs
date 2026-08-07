use ghost_codex::{MixingAgent, MockMixingAgent};
use ghost_core::prompt::PluginCapabilitySummary;
use ghost_core::{
    analyze_audio, build_prompt_bundle, read_wav, validate_mix_plan, AnalysisConfig, UserIntent,
};
use ghost_host::{HostedChain, MockFabFilterChain};

pub const DEFAULT_EDITOR_WIDTH: u32 = 900;
pub const DEFAULT_EDITOR_HEIGHT: u32 = 700;

pub struct GhostUi {
    input_path: String,
    prompt: String,
    profile: usize,
    status: String,
    result: String,
}

impl Default for GhostUi {
    fn default() -> Self {
        Self {
            input_path: "fixtures/muddy_bass.wav".into(),
            prompt: "Tighten the low mids while preserving punch.".into(),
            profile: 2,
            status: "Ready".into(),
            result: String::new(),
        }
    }
}

impl GhostUi {
    fn run(&mut self) {
        let outcome = (|| -> Result<String, Box<dyn std::error::Error>> {
            let audio = read_wav(&self.input_path)?;
            let config = match self.profile {
                0 => AnalysisConfig::live(),
                1 => AnalysisConfig::high(),
                _ => AnalysisConfig::maximum(),
            };
            let analysis = analyze_audio(&self.input_path, &audio, &config)?;
            let capabilities = vec![PluginCapabilitySummary {
                plugin: "Mock Pro-Q 4 + Pro-C 3 chain".into(),
                version: "0.1".into(),
                supported_operations: vec!["bell EQ".into(), "compression".into()],
                safety_notes: Vec::new(),
            }];
            let bundle = build_prompt_bundle(
                include_str!("../../../prompts/system.md"),
                UserIntent::Freeform {
                    prompt: self.prompt.clone(),
                },
                &analysis,
                &capabilities,
            )?;
            let mut agent = MockMixingAgent;
            let plan = agent.propose(&bundle)?;
            validate_mix_plan(&plan)?;
            let mut host = MockFabFilterChain::default();
            let processed = host.render(&audio, &plan)?;
            let processed_analysis = analyze_audio("processed", &processed, &config)?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "plan": plan,
                "before": {
                    "rms_dbfs": analysis.signal.loudness.rms_dbfs,
                    "crest_db": analysis.signal.loudness.crest_factor_db,
                    "centroid_hz": analysis.signal.spectrum.centroid_hz,
                    "low_mid_db": analysis.signal.spectrum.bands.low_mid_db
                },
                "after": {
                    "rms_dbfs": processed_analysis.signal.loudness.rms_dbfs,
                    "crest_db": processed_analysis.signal.loudness.crest_factor_db,
                    "centroid_hz": processed_analysis.signal.spectrum.centroid_hz,
                    "low_mid_db": processed_analysis.signal.spectrum.bands.low_mid_db
                }
            }))?)
        })();
        match outcome {
            Ok(result) => {
                self.status = "Complete".into();
                self.result = result;
            }
            Err(error) => {
                self.status = "Failed".into();
                self.result = error.to_string();
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.show_contents(ui);
        });
    }

    pub fn show_contents(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ghost Agent Host Lab");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("WAV file");
            ui.text_edit_singleline(&mut self.input_path);
        });
        ui.horizontal(|ui| {
            ui.label("Quality");
            egui::ComboBox::from_id_salt("quality")
                .selected_text(["Live", "High", "Maximum"][self.profile])
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.profile, 0, "Live");
                    ui.selectable_value(&mut self.profile, 1, "High");
                    ui.selectable_value(&mut self.profile, 2, "Maximum");
                });
        });
        ui.label("Intent");
        ui.text_edit_multiline(&mut self.prompt);
        if ui.button("Listen / Analyze / Propose").clicked() {
            self.status = "Working".into();
            self.run();
        }
        ui.label(format!("Status: {}", self.status));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.result)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(30),
            );
        });
    }
}
