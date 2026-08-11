use std::fs::File;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{AudioBuffer, AudioError};

/// Decode an enabled media format into the planar f32 representation used by analysis.
pub fn read_audio(path: impl AsRef<Path>) -> Result<AudioBuffer, AudioError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| AudioError::Decode(error.to_string()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AudioError::Decode("media has no default audio track".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let mut sample_rate = None;
    let mut channels: Vec<Vec<f32>> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::ResetRequired) => {
                return Err(AudioError::Decode("decoder reset is not supported".into()))
            }
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(AudioError::Decode(error.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(AudioError::Decode(error.to_string())),
        };
        append(decoded, &mut sample_rate, &mut channels)?;
    }

    let audio = AudioBuffer {
        sample_rate: sample_rate
            .ok_or_else(|| AudioError::Decode("media had no samples".into()))?,
        channels,
    };
    audio.validate()?;
    Ok(audio)
}

fn append(
    decoded: AudioBufferRef<'_>,
    sample_rate: &mut Option<u32>,
    channels: &mut Vec<Vec<f32>>,
) -> Result<(), AudioError> {
    let spec = *decoded.spec();
    if sample_rate.is_some_and(|value| value != spec.rate) {
        return Err(AudioError::Decode(
            "sample rate changed during decode".into(),
        ));
    }
    *sample_rate = Some(spec.rate);
    let channel_count = spec.channels.count();
    if channels.is_empty() {
        channels.resize_with(channel_count, Vec::new);
    } else if channels.len() != channel_count {
        return Err(AudioError::Decode(
            "channel layout changed during decode".into(),
        ));
    }
    let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
    samples.copy_interleaved_ref(decoded);
    for frame in samples.samples().chunks_exact(channel_count) {
        for (destination, sample) in channels.iter_mut().zip(frame) {
            destination.push(*sample);
        }
    }
    Ok(())
}
