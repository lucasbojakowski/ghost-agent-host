//! Format-neutral in-memory audio and file adapters.

use std::path::Path;

use thiserror::Error;

mod decoder;

pub use decoder::read_audio;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("wav error: {0}")]
    Wav(#[from] hound::Error),
    #[error("unsupported WAV bit depth {0}")]
    UnsupportedBitDepth(u16),
    #[error("audio has no channels")]
    NoChannels,
    #[error("channel lengths differ")]
    ChannelLengthMismatch,
    #[error("media decode error: {0}")]
    Decode(String),
}

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl AudioBuffer {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    pub fn duration_seconds(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }

    pub fn validate(&self) -> Result<(), AudioError> {
        if self.channels.is_empty() {
            return Err(AudioError::NoChannels);
        }
        let frames = self.frames();
        if self.channels.iter().any(|channel| channel.len() != frames) {
            return Err(AudioError::ChannelLengthMismatch);
        }
        Ok(())
    }

    pub fn mono_mix(&self) -> Vec<f32> {
        let frames = self.frames();
        let gain = 1.0 / self.channels.len() as f32;
        let mut mono = vec![0.0; frames];
        for channel in &self.channels {
            for (output, sample) in mono.iter_mut().zip(channel) {
                *output += sample * gain;
            }
        }
        mono
    }

    pub fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.frames() * self.channels.len());
        for frame in 0..self.frames() {
            for channel in &self.channels {
                out.push(channel[frame]);
            }
        }
        out
    }
}

pub fn read_wav(path: impl AsRef<Path>) -> Result<AudioBuffer, AudioError> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channel_count = spec.channels as usize;
    if channel_count == 0 {
        return Err(AudioError::NoChannels);
    }

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => match spec.bits_per_sample {
            8 => reader
                .samples::<i8>()
                .map(|value| value.map(|sample| sample as f32 / i8::MAX as f32))
                .collect::<Result<Vec<_>, _>>()?,
            16 => reader
                .samples::<i16>()
                .map(|value| value.map(|sample| sample as f32 / i16::MAX as f32))
                .collect::<Result<Vec<_>, _>>()?,
            24 | 32 => {
                let scale = ((1_i64 << (spec.bits_per_sample - 1)) - 1) as f32;
                reader
                    .samples::<i32>()
                    .map(|value| value.map(|sample| sample as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()?
            }
            depth => return Err(AudioError::UnsupportedBitDepth(depth)),
        },
    };

    let frames = samples.len() / channel_count;
    let mut channels = vec![Vec::with_capacity(frames); channel_count];
    for frame in samples.chunks_exact(channel_count) {
        for (channel, sample) in channels.iter_mut().zip(frame) {
            channel.push(*sample);
        }
    }

    let audio = AudioBuffer {
        sample_rate: spec.sample_rate,
        channels,
    };
    audio.validate()?;
    Ok(audio)
}

pub fn write_wav_f32(path: impl AsRef<Path>, audio: &AudioBuffer) -> Result<(), AudioError> {
    audio.validate()?;
    let spec = hound::WavSpec {
        channels: audio.channels.len() as u16,
        sample_rate: audio.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for frame in 0..audio.frames() {
        for channel in &audio.channels {
            writer.write_sample(channel[frame])?;
        }
    }
    writer.finalize()?;
    Ok(())
}
