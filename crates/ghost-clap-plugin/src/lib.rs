//! Minimal DAW-loadable Ghost Tap CLAP plugin.
//!
//! Ghost Tap is deliberately boring: one stereo input, one stereo output, transparent passthrough,
//! transport publication, and bounded capture into [`ghost_core::RealtimeCaptureBuffer`]. It does
//! not host child plugins, expose a GUI, publish parameters, or perform filesystem work on the audio
//! callback. A small non-realtime worker consumes capture commands from the Ghost Tap filesystem
//! protocol and commits completed WAV files for the external Ghost application.

use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use clack_common::events::event_types::{TransportEvent, TransportFlags};
use clack_common::utils::ClapId;
use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
    PluginAudioPortsImpl,
};
use clack_plugin::plugin::features::{ANALYZER, AUDIO_EFFECT, STEREO};
use clack_plugin::prelude::*;
use ghost_core::{
    publish_capture_artifact, publish_tap_status, read_capture_command, unix_ms, write_wav_f32,
    AtomicDawState, CaptureTriggerConfig, DawTransportSnapshot, RealtimeCaptureBuffer,
    RealtimeCaptureState, TapCaptureArtifact, TapPaths, TapStatus, TAP_PLUGIN_ID, TAP_PROTOCOL,
};

pub struct GhostTapPlugin;
pub struct TapMainThread;

const CAPTURE_CAPACITY_FRAMES: usize = 1_152_000;
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STATUS_INTERVAL: Duration = Duration::from_millis(500);

static NEXT_INSTANCE_ID: AtomicU32 = AtomicU32::new(0);

pub struct GhostShared {
    instance_id: u32,
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    _worker: Option<CaptureWorker>,
}

impl GhostShared {
    fn new() -> Self {
        let instance_id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
        let daw = Arc::new(AtomicDawState::default());
        let capture = Arc::new(RealtimeCaptureBuffer::new(CAPTURE_CAPACITY_FRAMES));
        let worker = CaptureWorker::spawn(instance_id, Arc::clone(&daw), Arc::clone(&capture));
        Self {
            instance_id,
            daw,
            capture,
            _worker: worker,
        }
    }
}

impl PluginShared<'_> for GhostShared {}

impl PluginMainThread<'_, GhostShared> for TapMainThread {}

impl PluginAudioPortsImpl for TapMainThread {
    fn count(&mut self, _is_input: bool) -> u32 {
        1
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        if index != 0 {
            return;
        }
        writer.set(&AudioPortInfo {
            id: ClapId::new(0),
            name: if is_input {
                b"Stereo input"
            } else {
                b"Stereo output"
            },
            channel_count: 2,
            flags: AudioPortFlags::IS_MAIN,
            port_type: Some(AudioPortType::STEREO),
            in_place_pair: ClapId::new(0).into(),
        });
    }
}

impl Plugin for GhostTapPlugin {
    type AudioProcessor<'a> = GhostAudioProcessor;
    type Shared<'a> = GhostShared;
    type MainThread<'a> = TapMainThread;

    fn declare_extensions(
        builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        builder.register::<PluginAudioPorts>();
    }
}

impl DefaultPluginFactory for GhostTapPlugin {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new(TAP_PLUGIN_ID, "Ghost Tap")
            .with_vendor("Konko")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_description("Low-overhead passthrough capture tap for Ghost")
            .with_features([AUDIO_EFFECT, ANALYZER, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(GhostShared::new())
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(TapMainThread)
    }
}

pub struct GhostAudioProcessor {
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
}

impl<'a> PluginAudioProcessor<'a, GhostShared, TapMainThread> for GhostAudioProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut TapMainThread,
        shared: &'a GhostShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        shared.daw.set_audio_configuration(
            audio_config.sample_rate,
            audio_config.min_frames_count,
            audio_config.max_frames_count,
        );
        Ok(Self {
            daw: Arc::clone(&shared.daw),
            capture: Arc::clone(&shared.capture),
        })
    }

    fn process(
        &mut self,
        process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        self.daw
            .publish_transport(transport_snapshot(process.steady_time, process.transport));

        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };
            let mut channels = channel_pairs.into_iter();
            match (channels.next(), channels.next()) {
                (
                    Some(ChannelPair::InputOutput(left_in, left_out)),
                    Some(ChannelPair::InputOutput(right_in, right_out)),
                ) => {
                    let frames = left_in
                        .len()
                        .min(right_in.len())
                        .min(left_out.len())
                        .min(right_out.len());
                    left_out[..frames].copy_from_slice(&left_in[..frames]);
                    right_out[..frames].copy_from_slice(&right_in[..frames]);
                    self.capture.push_stereo(
                        &left_in[..frames],
                        &right_in[..frames],
                        &left_out[..frames],
                        &right_out[..frames],
                    );
                }
                (Some(ChannelPair::InPlace(left)), Some(ChannelPair::InPlace(right))) => {
                    let frames = left.len().min(right.len());
                    self.capture.push_stereo(
                        &left[..frames],
                        &right[..frames],
                        &left[..frames],
                        &right[..frames],
                    );
                }
                (left, right) => {
                    if let Some(pair) = left {
                        passthrough(pair);
                    }
                    if let Some(pair) = right {
                        passthrough(pair);
                    }
                }
            }
            for pair in channels {
                passthrough(pair);
            }
        }
        Ok(ProcessStatus::Continue)
    }

    fn deactivate(self, _main_thread: &mut TapMainThread) {
        self.daw.clear_audio_configuration();
        self.capture.cancel();
    }
}

fn passthrough(pair: ChannelPair<'_, f32>) {
    match pair {
        ChannelPair::InputOnly(_) | ChannelPair::InPlace(_) => {}
        ChannelPair::OutputOnly(output) => output.fill(0.0),
        ChannelPair::InputOutput(input, output) => {
            let frames = input.len().min(output.len());
            output[..frames].copy_from_slice(&input[..frames]);
            if output.len() > frames {
                output[frames..].fill(0.0);
            }
        }
    }
}

struct CaptureWorker {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl CaptureWorker {
    fn spawn(
        instance_id: u32,
        daw: Arc<AtomicDawState>,
        capture: Arc<RealtimeCaptureBuffer>,
    ) -> Option<Self> {
        let paths = TapPaths::for_instance(std::process::id(), instance_id).ok()?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name(format!("ghost-tap-{instance_id}"))
            .spawn(move || capture_worker_loop(instance_id, paths, daw, capture, worker_stop))
            .ok()?;
        Some(Self {
            stop,
            join: Some(join),
        })
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn capture_worker_loop(
    instance_id: u32,
    paths: TapPaths,
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    stop: Arc<AtomicBool>,
) {
    let process_id = std::process::id();
    let mut last_request_id = 0_u64;
    let mut active_request_id = None;
    let mut last_status = std::time::Instant::now() - STATUS_INTERVAL;
    let mut last_error = None::<String>;

    while !stop.load(Ordering::Acquire) {
        if let Ok(command) = read_capture_command(&paths.command) {
            if command.protocol == TAP_PROTOCOL && command.request_id != last_request_id {
                last_request_id = command.request_id;
                let audio = daw.audio_configuration();
                match audio.sample_rate {
                    Some(rate) if rate.is_finite() && rate > 0.0 => {
                        let frames = (command.duration_seconds * rate).round() as usize;
                        let trigger = CaptureTriggerConfig {
                            threshold_dbfs: command.threshold_dbfs,
                            persistence_blocks: command.persistence_blocks,
                            pre_roll_ms: command.pre_roll_ms,
                        };
                        if frames == 0 || frames > capture.capacity_frames() {
                            last_error = Some(format!(
                                "capture requested {frames} frames but tap capacity is {}",
                                capture.capacity_frames()
                            ));
                        } else if !capture.configure_trigger(trigger) {
                            last_error = Some("capture trigger configuration was rejected".into());
                        } else if !capture.arm_tap(frames, rate.round() as u32, "output") {
                            last_error = Some("capture could not be armed while another capture is recording".into());
                        } else {
                            active_request_id = Some(command.request_id);
                            last_error = None;
                        }
                    }
                    _ => {
                        last_error = Some("Ghost Tap is not currently audio-active in the DAW".into());
                    }
                }
            }
        }

        if capture.state() == RealtimeCaptureState::Complete {
            if let Some(request_id) = active_request_id.take() {
                match capture.snapshot(daw.transport()) {
                    Some(snapshot) => {
                        let wav_path = paths.wav_for_request(request_id);
                        let temporary_wav = wav_path.with_extension("tmp.wav");
                        let commit = (|| -> Result<(), String> {
                            write_wav_f32(&temporary_wav, &snapshot.output)
                                .map_err(|error| error.to_string())?;
                            if wav_path.exists() {
                                let _ = fs::remove_file(&wav_path);
                            }
                            fs::rename(&temporary_wav, &wav_path)
                                .map_err(|error| error.to_string())?;
                            let frames = snapshot.output.frames();
                            let artifact = TapCaptureArtifact {
                                protocol: TAP_PROTOCOL.into(),
                                request_id,
                                process_id,
                                instance_id,
                                sample_rate: snapshot.output.sample_rate,
                                frames,
                                duration_seconds: snapshot.output.duration_seconds(),
                                wav_path,
                                transport: snapshot.transport,
                                completed_unix_ms: unix_ms(),
                            };
                            publish_capture_artifact(&paths.artifact, &artifact)
                                .map_err(|error| error.to_string())
                        })();
                        last_error = commit.err();
                    }
                    None => last_error = Some("complete capture could not be snapshotted".into()),
                }
            }
        }

        if last_status.elapsed() >= STATUS_INTERVAL {
            let audio = daw.audio_configuration();
            let status = TapStatus {
                protocol: TAP_PROTOCOL.into(),
                plugin_id: TAP_PLUGIN_ID.into(),
                process_id,
                instance_id,
                sample_rate: audio.sample_rate,
                maximum_block_frames: audio.maximum_frames,
                capture_state: capture.state(),
                active_request_id,
                command_path: paths.command.clone(),
                artifact_path: paths.artifact.clone(),
                updated_unix_ms: unix_ms(),
                last_error: last_error.clone(),
            };
            let _ = publish_tap_status(&paths.status, &status);
            last_status = std::time::Instant::now();
        }

        thread::sleep(WORKER_POLL_INTERVAL);
    }

    let _ = fs::remove_file(paths.status);
}

fn transport_snapshot(
    steady_time: Option<u64>,
    transport: Option<&TransportEvent>,
) -> DawTransportSnapshot {
    let Some(transport) = transport else {
        return DawTransportSnapshot {
            steady_sample_time: steady_time,
            ..DawTransportSnapshot::default()
        };
    };
    let flags = transport.flags;
    DawTransportSnapshot {
        steady_sample_time: steady_time,
        tempo_bpm: flags
            .contains(TransportFlags::HAS_TEMPO)
            .then_some(transport.tempo),
        tempo_increment_bpm_per_sample: flags
            .contains(TransportFlags::HAS_TEMPO)
            .then_some(transport.tempo_inc),
        song_position_beats: flags
            .contains(TransportFlags::HAS_BEATS_TIMELINE)
            .then(|| transport.song_pos_beats.to_float()),
        song_position_seconds: flags
            .contains(TransportFlags::HAS_SECONDS_TIMELINE)
            .then(|| transport.song_pos_seconds.to_float()),
        bar_start_beats: flags
            .contains(TransportFlags::HAS_BEATS_TIMELINE)
            .then(|| transport.bar_start.to_float()),
        bar_number: flags
            .contains(TransportFlags::HAS_BEATS_TIMELINE)
            .then_some(transport.bar_number),
        time_signature: flags
            .contains(TransportFlags::HAS_TIME_SIGNATURE)
            .then_some((
                transport.time_signature_numerator,
                transport.time_signature_denominator,
            )),
        loop_beats: (flags.contains(TransportFlags::IS_LOOP_ACTIVE)
            && flags.contains(TransportFlags::HAS_BEATS_TIMELINE))
        .then(|| {
            (
                transport.loop_start_beats.to_float(),
                transport.loop_end_beats.to_float(),
            )
        }),
        loop_seconds: (flags.contains(TransportFlags::IS_LOOP_ACTIVE)
            && flags.contains(TransportFlags::HAS_SECONDS_TIMELINE))
        .then(|| {
            (
                transport.loop_start_seconds.to_float(),
                transport.loop_end_seconds.to_float(),
            )
        }),
        playing: flags.contains(TransportFlags::IS_PLAYING),
        recording: flags.contains(TransportFlags::IS_RECORDING),
        looping: flags.contains(TransportFlags::IS_LOOP_ACTIVE),
        within_pre_roll: flags.contains(TransportFlags::IS_WITHIN_PRE_ROLL),
    }
}

clack_export_entry!(SinglePluginEntry<GhostTapPlugin>);
