pub mod analysis;
pub mod capture;
pub mod audio;
pub mod mock_dsp;
pub mod model;
pub mod prompt;
pub mod validation;

pub use analysis::{analyze_audio, AnalysisError};
pub use audio::{read_wav, write_wav_f32, AudioBuffer, AudioError};
pub use model::*;
pub use prompt::{build_prompt_bundle, PromptBundle};
pub use validation::{validate_mix_plan, ValidationError};
