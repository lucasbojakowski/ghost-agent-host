//! DAW-owned activation, transport, and bounded realtime capture primitives.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::audio::AudioBuffer;

const IDLE: u8 = 0;
const ARMED: u8 = 1;
const RECORDING: u8 = 2;
const COMPLETE: u8 = 3;

const HAS_STEADY_TIME: u32 = 1 << 0;
const HAS_TEMPO: u32 = 1 << 1;
const HAS_BEATS: u32 = 1 << 2;
const HAS_SECONDS: u32 = 1 << 3;
const HAS_TIME_SIGNATURE: u32 = 1 << 4;
const PLAYING: u32 = 1 << 5;
const RECORDING_FLAG: u32 = 1 << 6;
const LOOPING: u32 = 1 << 7;
const PRE_ROLL: u32 = 1 << 8;
const HAS_TEMPO_INCREMENT: u32 = 1 << 9;
const HAS_LOOP_BEATS: u32 = 1 << 10;
const HAS_LOOP_SECONDS: u32 = 1 << 11;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct DawTransportSnapshot {
    pub steady_sample_time: Option<u64>,
    pub tempo_bpm: Option<f64>,
    pub tempo_increment_bpm_per_sample: Option<f64>,
    pub song_position_beats: Option<f64>,
    pub song_position_seconds: Option<f64>,
    pub bar_start_beats: Option<f64>,
    pub bar_number: Option<i32>,
    pub time_signature: Option<(u16, u16)>,
    pub loop_beats: Option<(f64, f64)>,
    pub loop_seconds: Option<(f64, f64)>,
    pub playing: bool,
    pub recording: bool,
    pub looping: bool,
    pub within_pre_roll: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct DawAudioConfiguration {
    pub sample_rate: Option<f64>,
    pub minimum_frames: Option<u32>,
    pub maximum_frames: Option<u32>,
}

/// Atomic transport projection. The sequence counter prevents readers from observing a mix of two
/// process callbacks while keeping publication allocation- and lock-free.
pub struct AtomicDawState {
    sequence: AtomicU64,
    sample_rate: AtomicU64,
    minimum_frames: AtomicU32,
    maximum_frames: AtomicU32,
    availability: AtomicU32,
    steady_time: AtomicU64,
    tempo: AtomicU64,
    tempo_increment: AtomicU64,
    song_beats: AtomicU64,
    song_seconds: AtomicU64,
    bar_start: AtomicU64,
    bar_number: AtomicU32,
    signature: AtomicU32,
    loop_start_beats: AtomicU64,
    loop_end_beats: AtomicU64,
    loop_start_seconds: AtomicU64,
    loop_end_seconds: AtomicU64,
}

#[derive(Default)]
pub struct AtomicGraphControl {
    bypass_mask: AtomicU64,
}

impl AtomicGraphControl {
    pub fn set_bypass_mask(&self, mask: u64) {
        self.bypass_mask.store(mask, Ordering::Release);
    }

    pub fn is_bypassed(&self, bit: u64) -> bool {
        self.bypass_mask.load(Ordering::Acquire) & bit != 0
    }
}

impl Default for AtomicDawState {
    fn default() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            sample_rate: AtomicU64::new(f64::NAN.to_bits()),
            minimum_frames: AtomicU32::new(0),
            maximum_frames: AtomicU32::new(0),
            availability: AtomicU32::new(0),
            steady_time: AtomicU64::new(0),
            tempo: AtomicU64::new(0),
            tempo_increment: AtomicU64::new(0),
            song_beats: AtomicU64::new(0),
            song_seconds: AtomicU64::new(0),
            bar_start: AtomicU64::new(0),
            bar_number: AtomicU32::new(0),
            signature: AtomicU32::new(0),
            loop_start_beats: AtomicU64::new(0),
            loop_end_beats: AtomicU64::new(0),
            loop_start_seconds: AtomicU64::new(0),
            loop_end_seconds: AtomicU64::new(0),
        }
    }
}

impl AtomicDawState {
    /// Monotonic process-publication generation for repaint and freshness tracking.
    pub fn transport_generation(&self) -> u64 {
        self.sequence.load(Ordering::Acquire) / 2
    }

    pub fn set_audio_configuration(&self, sample_rate: f64, minimum: u32, maximum: u32) {
        self.sample_rate
            .store(sample_rate.to_bits(), Ordering::Release);
        self.minimum_frames.store(minimum, Ordering::Release);
        self.maximum_frames.store(maximum, Ordering::Release);
    }

    pub fn clear_audio_configuration(&self) {
        self.sample_rate
            .store(f64::NAN.to_bits(), Ordering::Release);
        self.minimum_frames.store(0, Ordering::Release);
        self.maximum_frames.store(0, Ordering::Release);
    }

    pub fn audio_configuration(&self) -> DawAudioConfiguration {
        let rate = f64::from_bits(self.sample_rate.load(Ordering::Acquire));
        DawAudioConfiguration {
            sample_rate: rate.is_finite().then_some(rate),
            minimum_frames: nonzero(self.minimum_frames.load(Ordering::Acquire)),
            maximum_frames: nonzero(self.maximum_frames.load(Ordering::Acquire)),
        }
    }

    pub fn publish_transport(&self, snapshot: DawTransportSnapshot) {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        let mut flags = 0;
        store_optional_u64(
            &self.steady_time,
            snapshot.steady_sample_time,
            &mut flags,
            HAS_STEADY_TIME,
        );
        store_optional_f64(&self.tempo, snapshot.tempo_bpm, &mut flags, HAS_TEMPO);
        if let Some(value) = snapshot.tempo_increment_bpm_per_sample {
            self.tempo_increment
                .store(value.to_bits(), Ordering::Relaxed);
            flags |= HAS_TEMPO_INCREMENT;
        }
        store_optional_f64(
            &self.song_beats,
            snapshot.song_position_beats,
            &mut flags,
            HAS_BEATS,
        );
        store_optional_f64(
            &self.song_seconds,
            snapshot.song_position_seconds,
            &mut flags,
            HAS_SECONDS,
        );
        store_optional_f64(
            &self.bar_start,
            snapshot.bar_start_beats,
            &mut flags,
            HAS_BEATS,
        );
        self.bar_number.store(
            snapshot.bar_number.unwrap_or_default() as u32,
            Ordering::Relaxed,
        );
        if let Some((numerator, denominator)) = snapshot.time_signature {
            flags |= HAS_TIME_SIGNATURE;
            self.signature.store(
                u32::from(numerator) | (u32::from(denominator) << 16),
                Ordering::Relaxed,
            );
        }
        if let Some((start, end)) = snapshot.loop_beats {
            self.loop_start_beats
                .store(start.to_bits(), Ordering::Relaxed);
            self.loop_end_beats.store(end.to_bits(), Ordering::Relaxed);
            flags |= HAS_LOOP_BEATS;
        }
        if let Some((start, end)) = snapshot.loop_seconds {
            self.loop_start_seconds
                .store(start.to_bits(), Ordering::Relaxed);
            self.loop_end_seconds
                .store(end.to_bits(), Ordering::Relaxed);
            flags |= HAS_LOOP_SECONDS;
        }
        flags |= (snapshot.playing as u32) * PLAYING;
        flags |= (snapshot.recording as u32) * RECORDING_FLAG;
        flags |= (snapshot.looping as u32) * LOOPING;
        flags |= (snapshot.within_pre_roll as u32) * PRE_ROLL;
        self.availability.store(flags, Ordering::Release);
        self.sequence.fetch_add(1, Ordering::Release);
    }

    pub fn transport(&self) -> DawTransportSnapshot {
        loop {
            let before = self.sequence.load(Ordering::Acquire);
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let flags = self.availability.load(Ordering::Acquire);
            let snapshot = DawTransportSnapshot {
                steady_sample_time: flag(flags, HAS_STEADY_TIME)
                    .then(|| self.steady_time.load(Ordering::Relaxed)),
                tempo_bpm: load_optional_f64(&self.tempo, flags, HAS_TEMPO),
                tempo_increment_bpm_per_sample: flag(flags, HAS_TEMPO_INCREMENT)
                    .then(|| f64::from_bits(self.tempo_increment.load(Ordering::Relaxed))),
                song_position_beats: load_optional_f64(&self.song_beats, flags, HAS_BEATS),
                song_position_seconds: load_optional_f64(&self.song_seconds, flags, HAS_SECONDS),
                bar_start_beats: load_optional_f64(&self.bar_start, flags, HAS_BEATS),
                bar_number: flag(flags, HAS_BEATS)
                    .then(|| self.bar_number.load(Ordering::Relaxed) as i32),
                time_signature: flag(flags, HAS_TIME_SIGNATURE).then(|| {
                    let packed = self.signature.load(Ordering::Relaxed);
                    (packed as u16, (packed >> 16) as u16)
                }),
                loop_beats: flag(flags, HAS_LOOP_BEATS).then(|| {
                    (
                        f64::from_bits(self.loop_start_beats.load(Ordering::Relaxed)),
                        f64::from_bits(self.loop_end_beats.load(Ordering::Relaxed)),
                    )
                }),
                loop_seconds: flag(flags, HAS_LOOP_SECONDS).then(|| {
                    (
                        f64::from_bits(self.loop_start_seconds.load(Ordering::Relaxed)),
                        f64::from_bits(self.loop_end_seconds.load(Ordering::Relaxed)),
                    )
                }),
                playing: flag(flags, PLAYING),
                recording: flag(flags, RECORDING_FLAG),
                looping: flag(flags, LOOPING),
                within_pre_roll: flag(flags, PRE_ROLL),
            };
            if before == self.sequence.load(Ordering::Acquire) {
                return snapshot;
            }
        }
    }
}

fn nonzero(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

fn flag(flags: u32, value: u32) -> bool {
    flags & value != 0
}

fn store_optional_u64(target: &AtomicU64, value: Option<u64>, flags: &mut u32, bit: u32) {
    if let Some(value) = value {
        target.store(value, Ordering::Relaxed);
        *flags |= bit;
    }
}

fn store_optional_f64(target: &AtomicU64, value: Option<f64>, flags: &mut u32, bit: u32) {
    if let Some(value) = value.filter(|value| value.is_finite()) {
        target.store(value.to_bits(), Ordering::Relaxed);
        *flags |= bit;
    }
}

fn load_optional_f64(target: &AtomicU64, flags: u32, bit: u32) -> Option<f64> {
    flag(flags, bit).then(|| f64::from_bits(target.load(Ordering::Relaxed)))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeCaptureState {
    Idle,
    Armed,
    Recording,
    Complete,
}

#[derive(Debug, Clone)]
pub struct DawCaptureSnapshot {
    pub input: AudioBuffer,
    pub output: AudioBuffer,
    pub transport: DawTransportSnapshot,
    pub tap_key: u64,
}

/// Fixed-capacity stereo recorder shared by one UI/control producer and one audio-thread producer.
/// Samples use atomics so cancellation/re-arming can never race with snapshot reads.
pub struct RealtimeCaptureBuffer {
    capacity_frames: usize,
    state: AtomicU8,
    target_frames: AtomicUsize,
    written_frames: AtomicUsize,
    sample_rate: AtomicU32,
    tap_key: AtomicU64,
    captured_tap_key: AtomicU64,
    input_left: Vec<AtomicU32>,
    input_right: Vec<AtomicU32>,
    output_left: Vec<AtomicU32>,
    output_right: Vec<AtomicU32>,
}

impl RealtimeCaptureBuffer {
    pub fn new(capacity_frames: usize) -> Self {
        let channel = || (0..capacity_frames).map(|_| AtomicU32::new(0)).collect();
        Self {
            capacity_frames,
            state: AtomicU8::new(IDLE),
            target_frames: AtomicUsize::new(0),
            written_frames: AtomicUsize::new(0),
            sample_rate: AtomicU32::new(0),
            tap_key: AtomicU64::new(capture_tap_key("output")),
            captured_tap_key: AtomicU64::new(capture_tap_key("output")),
            input_left: channel(),
            input_right: channel(),
            output_left: channel(),
            output_right: channel(),
        }
    }

    pub fn capacity_frames(&self) -> usize {
        self.capacity_frames
    }

    pub fn state(&self) -> RealtimeCaptureState {
        match self.state.load(Ordering::Acquire) {
            ARMED => RealtimeCaptureState::Armed,
            RECORDING => RealtimeCaptureState::Recording,
            COMPLETE => RealtimeCaptureState::Complete,
            _ => RealtimeCaptureState::Idle,
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        (
            self.written_frames.load(Ordering::Acquire),
            self.target_frames.load(Ordering::Acquire),
        )
    }

    pub fn arm(&self, frames: usize, sample_rate: u32) -> bool {
        self.arm_tap(frames, sample_rate, "output")
    }

    pub fn arm_tap(&self, frames: usize, sample_rate: u32, tap_id: &str) -> bool {
        if frames == 0 || frames > self.capacity_frames || sample_rate == 0 {
            return false;
        }
        if matches!(self.state(), RealtimeCaptureState::Recording) {
            return false;
        }
        self.sample_rate.store(sample_rate, Ordering::Release);
        self.tap_key
            .store(capture_tap_key(tap_id), Ordering::Release);
        self.target_frames.store(frames, Ordering::Release);
        self.written_frames.store(0, Ordering::Release);
        self.state.store(ARMED, Ordering::Release);
        true
    }

    pub fn tap_key(&self) -> u64 {
        self.tap_key.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.state.store(IDLE, Ordering::Release);
        self.written_frames.store(0, Ordering::Release);
    }

    /// Copies at most the armed remainder. It performs no allocation, locking, or blocking.
    pub fn push_stereo(
        &self,
        input_left: &[f32],
        input_right: &[f32],
        output_left: &[f32],
        output_right: &[f32],
    ) {
        self.push_stereo_for_tap(
            input_left,
            input_right,
            output_left,
            output_right,
            self.tap_key(),
        );
    }

    pub fn push_stereo_for_tap(
        &self,
        input_left: &[f32],
        input_right: &[f32],
        output_left: &[f32],
        output_right: &[f32],
        actual_tap_key: u64,
    ) {
        let state = self.state.load(Ordering::Acquire);
        if state == ARMED {
            let _ =
                self.state
                    .compare_exchange(ARMED, RECORDING, Ordering::AcqRel, Ordering::Acquire);
        } else if state != RECORDING {
            return;
        }
        if self.state.load(Ordering::Acquire) != RECORDING {
            return;
        }
        self.captured_tap_key
            .store(actual_tap_key, Ordering::Release);
        let written = self.written_frames.load(Ordering::Relaxed);
        let target = self.target_frames.load(Ordering::Acquire);
        let frames = target
            .saturating_sub(written)
            .min(input_left.len())
            .min(input_right.len())
            .min(output_left.len())
            .min(output_right.len());
        for offset in 0..frames {
            let index = written + offset;
            self.input_left[index].store(input_left[offset].to_bits(), Ordering::Relaxed);
            self.input_right[index].store(input_right[offset].to_bits(), Ordering::Relaxed);
            self.output_left[index].store(output_left[offset].to_bits(), Ordering::Relaxed);
            self.output_right[index].store(output_right[offset].to_bits(), Ordering::Relaxed);
        }
        let total = written + frames;
        self.written_frames.store(total, Ordering::Release);
        if total >= target {
            self.state.store(COMPLETE, Ordering::Release);
        }
    }

    pub fn snapshot(&self, transport: DawTransportSnapshot) -> Option<DawCaptureSnapshot> {
        if self.state() != RealtimeCaptureState::Complete {
            return None;
        }
        let frames = self.written_frames.load(Ordering::Acquire);
        let sample_rate = self.sample_rate.load(Ordering::Acquire);
        let read = |source: &[AtomicU32]| {
            source[..frames]
                .iter()
                .map(|sample| f32::from_bits(sample.load(Ordering::Relaxed)))
                .collect()
        };
        Some(DawCaptureSnapshot {
            input: AudioBuffer {
                sample_rate,
                channels: vec![read(&self.input_left), read(&self.input_right)],
            },
            output: AudioBuffer {
                sample_rate,
                channels: vec![read(&self.output_left), read(&self.output_right)],
            },
            transport,
            tap_key: self.captured_tap_key.load(Ordering::Acquire),
        })
    }
}

/// Stable allocation-free key used to match a selected graph edge on the audio thread.
pub fn capture_tap_key(value: &str) -> u64 {
    capture_key_bytes(0xcbf29ce484222325, value.bytes())
}

pub fn capture_post_tap_key(node_id: &str) -> u64 {
    let hash = capture_key_bytes(0xcbf29ce484222325, "post:".bytes());
    capture_key_bytes(hash, node_id.bytes())
}

fn capture_key_bytes(mut hash: u64, bytes: impl Iterator<Item = u8>) -> u64 {
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_available_transport_fields() {
        let state = AtomicDawState::default();
        assert_eq!(state.transport_generation(), 0);
        state.set_audio_configuration(48_000.0, 16, 1024);
        state.publish_transport(DawTransportSnapshot {
            tempo_bpm: Some(127.5),
            tempo_increment_bpm_per_sample: Some(0.0001),
            song_position_beats: Some(12.25),
            time_signature: Some((7, 8)),
            loop_beats: Some((8.0, 16.0)),
            within_pre_roll: true,
            playing: true,
            ..DawTransportSnapshot::default()
        });
        assert_eq!(state.transport_generation(), 1);
        assert_eq!(state.audio_configuration().sample_rate, Some(48_000.0));
        let transport = state.transport();
        assert_eq!(transport.tempo_bpm, Some(127.5));
        assert_eq!(transport.tempo_increment_bpm_per_sample, Some(0.0001));
        assert_eq!(transport.time_signature, Some((7, 8)));
        assert_eq!(transport.loop_beats, Some((8.0, 16.0)));
        assert!(transport.within_pre_roll);
        assert!(transport.playing);
        assert_eq!(transport.song_position_seconds, None);
    }

    #[test]
    fn records_bounded_input_and_output_without_start_call() {
        let capture = RealtimeCaptureBuffer::new(8);
        assert!(capture.arm(3, 48_000));
        capture.push_stereo(&[0.1, 0.2], &[0.3, 0.4], &[0.5, 0.6], &[0.7, 0.8]);
        capture.push_stereo(&[0.9, 1.0], &[0.8, 0.7], &[0.6, 0.5], &[0.4, 0.3]);
        assert_eq!(capture.state(), RealtimeCaptureState::Complete);
        let snapshot = capture.snapshot(DawTransportSnapshot::default()).unwrap();
        assert_eq!(snapshot.input.channels[0], vec![0.1, 0.2, 0.9]);
        assert_eq!(snapshot.output.channels[1], vec![0.7, 0.8, 0.4]);
    }
}
