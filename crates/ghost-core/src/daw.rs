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

const MAX_CAPTURE_PRE_ROLL_FRAMES: usize = 38_400;
const DEFAULT_TRIGGER_DBFS: f32 = -50.0;
const DEFAULT_TRIGGER_PERSISTENCE_BLOCKS: u32 = 2;
const DEFAULT_PRE_ROLL_MS: u32 = 75;
const PEAK_GUARD_ABOVE_RMS_DB: f32 = 8.0;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CaptureTriggerConfig {
    pub threshold_dbfs: f32,
    pub persistence_blocks: u32,
    pub pre_roll_ms: u32,
}

impl Default for CaptureTriggerConfig {
    fn default() -> Self {
        Self {
            threshold_dbfs: DEFAULT_TRIGGER_DBFS,
            persistence_blocks: DEFAULT_TRIGGER_PERSISTENCE_BLOCKS,
            pre_roll_ms: DEFAULT_PRE_ROLL_MS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DawCaptureSnapshot {
    pub input: AudioBuffer,
    pub output: AudioBuffer,
    pub transport: DawTransportSnapshot,
    pub tap_key: u64,
}

/// Fixed-capacity stereo recorder shared by one UI/control producer and one audio-thread producer.
/// Samples use atomics so cancellation/re-arming can never race with snapshot reads. While armed,
/// a bounded ring retains pre-roll and a realtime-safe RMS/peak detector waits for actual signal.
pub struct RealtimeCaptureBuffer {
    capacity_frames: usize,
    state: AtomicU8,
    target_frames: AtomicUsize,
    written_frames: AtomicUsize,
    sample_rate: AtomicU32,
    tap_key: AtomicU64,
    captured_tap_key: AtomicU64,
    trigger_threshold_dbfs: AtomicU32,
    trigger_persistence_blocks: AtomicU32,
    trigger_blocks_seen: AtomicU32,
    pre_roll_ms: AtomicU32,
    pre_roll_write_index: AtomicUsize,
    pre_roll_filled: AtomicUsize,
    captured_pre_roll_frames: AtomicUsize,
    captured_pre_roll_end: AtomicUsize,
    pre_input_left: Vec<AtomicU32>,
    pre_input_right: Vec<AtomicU32>,
    pre_output_left: Vec<AtomicU32>,
    pre_output_right: Vec<AtomicU32>,
    input_left: Vec<AtomicU32>,
    input_right: Vec<AtomicU32>,
    output_left: Vec<AtomicU32>,
    output_right: Vec<AtomicU32>,
}

impl RealtimeCaptureBuffer {
    pub fn new(capacity_frames: usize) -> Self {
        let channel = || (0..capacity_frames).map(|_| AtomicU32::new(0)).collect();
        let pre_roll_channel = || {
            (0..MAX_CAPTURE_PRE_ROLL_FRAMES)
                .map(|_| AtomicU32::new(0))
                .collect()
        };
        let trigger = CaptureTriggerConfig::default();
        Self {
            capacity_frames,
            state: AtomicU8::new(IDLE),
            target_frames: AtomicUsize::new(0),
            written_frames: AtomicUsize::new(0),
            sample_rate: AtomicU32::new(0),
            tap_key: AtomicU64::new(capture_tap_key("output")),
            captured_tap_key: AtomicU64::new(capture_tap_key("output")),
            trigger_threshold_dbfs: AtomicU32::new(trigger.threshold_dbfs.to_bits()),
            trigger_persistence_blocks: AtomicU32::new(trigger.persistence_blocks),
            trigger_blocks_seen: AtomicU32::new(0),
            pre_roll_ms: AtomicU32::new(trigger.pre_roll_ms),
            pre_roll_write_index: AtomicUsize::new(0),
            pre_roll_filled: AtomicUsize::new(0),
            captured_pre_roll_frames: AtomicUsize::new(0),
            captured_pre_roll_end: AtomicUsize::new(0),
            pre_input_left: pre_roll_channel(),
            pre_input_right: pre_roll_channel(),
            pre_output_left: pre_roll_channel(),
            pre_output_right: pre_roll_channel(),
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

    pub fn trigger_config(&self) -> CaptureTriggerConfig {
        CaptureTriggerConfig {
            threshold_dbfs: f32::from_bits(self.trigger_threshold_dbfs.load(Ordering::Acquire)),
            persistence_blocks: self.trigger_persistence_blocks.load(Ordering::Acquire),
            pre_roll_ms: self.pre_roll_ms.load(Ordering::Acquire),
        }
    }

    pub fn configure_trigger(&self, config: CaptureTriggerConfig) -> bool {
        if !config.threshold_dbfs.is_finite()
            || config.threshold_dbfs > 0.0
            || config.threshold_dbfs < -120.0
            || config.persistence_blocks == 0
            || config.pre_roll_ms > 500
            || matches!(self.state(), RealtimeCaptureState::Recording)
        {
            return false;
        }
        self.trigger_threshold_dbfs
            .store(config.threshold_dbfs.to_bits(), Ordering::Release);
        self.trigger_persistence_blocks
            .store(config.persistence_blocks, Ordering::Release);
        self.pre_roll_ms.store(config.pre_roll_ms, Ordering::Release);
        true
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
        self.trigger_blocks_seen.store(0, Ordering::Release);
        self.pre_roll_write_index.store(0, Ordering::Release);
        self.pre_roll_filled.store(0, Ordering::Release);
        self.captured_pre_roll_frames.store(0, Ordering::Release);
        self.captured_pre_roll_end.store(0, Ordering::Release);
        self.state.store(ARMED, Ordering::Release);
        true
    }

    pub fn tap_key(&self) -> u64 {
        self.tap_key.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.state.store(IDLE, Ordering::Release);
        self.written_frames.store(0, Ordering::Release);
        self.trigger_blocks_seen.store(0, Ordering::Release);
        self.pre_roll_filled.store(0, Ordering::Release);
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
        let frames = input_left
            .len()
            .min(input_right.len())
            .min(output_left.len())
            .min(output_right.len());
        if frames == 0 {
            return;
        }

        match self.state.load(Ordering::Acquire) {
            ARMED => {
                self.push_pre_roll(
                    input_left,
                    input_right,
                    output_left,
                    output_right,
                    frames,
                );
                if !self.signal_triggered(output_left, output_right, frames) {
                    return;
                }
                self.captured_tap_key
                    .store(actual_tap_key, Ordering::Release);
                let target = self.target_frames.load(Ordering::Acquire);
                let pre_roll = self
                    .pre_roll_filled
                    .load(Ordering::Acquire)
                    .min(self.configured_pre_roll_frames())
                    .min(target);
                self.captured_pre_roll_frames
                    .store(pre_roll, Ordering::Release);
                self.captured_pre_roll_end.store(
                    self.pre_roll_write_index.load(Ordering::Acquire),
                    Ordering::Release,
                );
                self.written_frames.store(pre_roll, Ordering::Release);
                if pre_roll >= target {
                    self.state.store(COMPLETE, Ordering::Release);
                } else {
                    self.state.store(RECORDING, Ordering::Release);
                }
                return;
            }
            RECORDING => {}
            _ => return,
        }

        self.captured_tap_key
            .store(actual_tap_key, Ordering::Release);
        let written = self.written_frames.load(Ordering::Relaxed);
        let target = self.target_frames.load(Ordering::Acquire);
        let pre_roll = self.captured_pre_roll_frames.load(Ordering::Acquire);
        let copy_frames = target
            .saturating_sub(written)
            .min(frames)
            .min(self.capacity_frames.saturating_sub(written.saturating_sub(pre_roll)));
        let post_start = written.saturating_sub(pre_roll);
        for offset in 0..copy_frames {
            let index = post_start + offset;
            self.input_left[index].store(input_left[offset].to_bits(), Ordering::Relaxed);
            self.input_right[index].store(input_right[offset].to_bits(), Ordering::Relaxed);
            self.output_left[index].store(output_left[offset].to_bits(), Ordering::Relaxed);
            self.output_right[index].store(output_right[offset].to_bits(), Ordering::Relaxed);
        }
        let total = written + copy_frames;
        self.written_frames.store(total, Ordering::Release);
        if total >= target {
            self.state.store(COMPLETE, Ordering::Release);
        }
    }

    fn configured_pre_roll_frames(&self) -> usize {
        let sample_rate = self.sample_rate.load(Ordering::Acquire) as usize;
        let milliseconds = self.pre_roll_ms.load(Ordering::Acquire) as usize;
        sample_rate
            .saturating_mul(milliseconds)
            .saturating_div(1_000)
            .min(MAX_CAPTURE_PRE_ROLL_FRAMES)
    }

    fn push_pre_roll(
        &self,
        input_left: &[f32],
        input_right: &[f32],
        output_left: &[f32],
        output_right: &[f32],
        frames: usize,
    ) {
        let configured = self.configured_pre_roll_frames();
        if configured == 0 {
            return;
        }
        let mut write = self.pre_roll_write_index.load(Ordering::Relaxed) % configured;
        for offset in 0..frames {
            self.pre_input_left[write].store(input_left[offset].to_bits(), Ordering::Relaxed);
            self.pre_input_right[write].store(input_right[offset].to_bits(), Ordering::Relaxed);
            self.pre_output_left[write].store(output_left[offset].to_bits(), Ordering::Relaxed);
            self.pre_output_right[write].store(output_right[offset].to_bits(), Ordering::Relaxed);
            write += 1;
            if write == configured {
                write = 0;
            }
        }
        let filled = self
            .pre_roll_filled
            .load(Ordering::Relaxed)
            .saturating_add(frames)
            .min(configured);
        self.pre_roll_filled.store(filled, Ordering::Release);
        self.pre_roll_write_index.store(write, Ordering::Release);
    }

    fn signal_triggered(&self, left: &[f32], right: &[f32], frames: usize) -> bool {
        let mut sum_squares = 0.0_f64;
        let mut peak = 0.0_f32;
        for index in 0..frames {
            let left_sample = left[index];
            let right_sample = right[index];
            sum_squares += f64::from(left_sample) * f64::from(left_sample)
                + f64::from(right_sample) * f64::from(right_sample);
            peak = peak.max(left_sample.abs()).max(right_sample.abs());
        }
        let rms = (sum_squares / (frames as f64 * 2.0)).sqrt() as f32;
        let threshold_db = f32::from_bits(self.trigger_threshold_dbfs.load(Ordering::Acquire));
        let threshold = 10.0_f32.powf(threshold_db / 20.0);
        let peak_guard = 10.0_f32.powf((threshold_db + PEAK_GUARD_ABOVE_RMS_DB) / 20.0);
        let candidate = rms >= threshold || peak >= peak_guard;
        let seen = if candidate {
            self.trigger_blocks_seen.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            self.trigger_blocks_seen.store(0, Ordering::Release);
            0
        };
        seen >= self.trigger_persistence_blocks.load(Ordering::Acquire)
    }

    pub fn snapshot(&self, transport: DawTransportSnapshot) -> Option<DawCaptureSnapshot> {
        if self.state() != RealtimeCaptureState::Complete {
            return None;
        }
        let frames = self.written_frames.load(Ordering::Acquire);
        let sample_rate = self.sample_rate.load(Ordering::Acquire);
        let pre_roll = self
            .captured_pre_roll_frames
            .load(Ordering::Acquire)
            .min(frames);
        let pre_end = self.captured_pre_roll_end.load(Ordering::Acquire);
        let configured = self.configured_pre_roll_frames().max(1);
        let post_frames = frames.saturating_sub(pre_roll);

        let read = |pre_source: &[AtomicU32], source: &[AtomicU32]| {
            let mut samples = Vec::with_capacity(frames);
            if pre_roll > 0 {
                let start = (pre_end + configured - (pre_roll % configured)) % configured;
                for offset in 0..pre_roll {
                    let index = (start + offset) % configured;
                    samples.push(f32::from_bits(pre_source[index].load(Ordering::Relaxed)));
                }
            }
            samples.extend(
                source[..post_frames]
                    .iter()
                    .map(|sample| f32::from_bits(sample.load(Ordering::Relaxed))),
            );
            samples
        };
        Some(DawCaptureSnapshot {
            input: AudioBuffer {
                sample_rate,
                channels: vec![
                    read(&self.pre_input_left, &self.input_left),
                    read(&self.pre_input_right, &self.input_right),
                ],
            },
            output: AudioBuffer {
                sample_rate,
                channels: vec![
                    read(&self.pre_output_left, &self.output_left),
                    read(&self.pre_output_right, &self.output_right),
                ],
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
    fn capture_waits_for_signal_and_includes_preroll() {
        let capture = RealtimeCaptureBuffer::new(8);
        assert!(capture.configure_trigger(CaptureTriggerConfig {
            threshold_dbfs: -40.0,
            persistence_blocks: 1,
            pre_roll_ms: 100,
        }));
        assert!(capture.arm(3, 48_000));
        capture.push_stereo(&[0.0, 0.0], &[0.0, 0.0], &[0.0, 0.0], &[0.0, 0.0]);
        assert_eq!(capture.state(), RealtimeCaptureState::Armed);
        capture.push_stereo(&[0.1, 0.2], &[0.3, 0.4], &[0.5, 0.6], &[0.7, 0.8]);
        assert_eq!(capture.state(), RealtimeCaptureState::Recording);
        capture.push_stereo(&[0.9], &[0.8], &[0.6], &[0.4]);
        assert_eq!(capture.state(), RealtimeCaptureState::Complete);
        let snapshot = capture.snapshot(DawTransportSnapshot::default()).unwrap();
        assert_eq!(snapshot.input.channels[0], vec![0.1, 0.2, 0.9]);
        assert_eq!(snapshot.output.channels[1], vec![0.7, 0.8, 0.4]);
    }
}
