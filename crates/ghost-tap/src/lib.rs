extern crate self as ghost_core;

mod audio {
    pub use ghost_audio::AudioBuffer;
}
mod plugin;
mod realtime;
mod tap;

pub use ghost_audio::write_wav_f32;
pub use realtime::{
    AtomicDawState, CaptureTriggerConfig, DawAudioConfiguration, DawCaptureSnapshot,
    DawTransportSnapshot, RealtimeCaptureBuffer, RealtimeCaptureState,
};
pub use tap::*;
