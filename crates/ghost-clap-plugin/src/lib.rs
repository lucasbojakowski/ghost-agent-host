//! DAW-loadable outer CLAP boundary.

use std::sync::{Arc, Mutex};

use clack_common::events::event_types::{TransportEvent, TransportFlags};
use clack_common::utils::ClapId;
#[cfg(not(target_os = "windows"))]
use clack_extensions::audio_ports::PluginAudioPortsImpl;
use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
};
#[cfg(target_os = "windows")]
use clack_extensions::params::{HostParams, PluginAudioProcessorParams, PluginParams};
use clack_plugin::plugin::features::{ANALYZER, AUDIO_EFFECT, STEREO};
use clack_plugin::prelude::*;
use ghost_core::{
    capture_tap_key, AtomicDawState, AtomicGraphControl, DawTransportSnapshot,
    RealtimeCaptureBuffer,
};
use ghost_host::{AudioBlock, NativeClapAudio, ProcessConfig};

#[cfg(target_os = "windows")]
mod child_window;
#[cfg(target_os = "windows")]
mod editor;

#[cfg(target_os = "windows")]
type MainThreadState = editor::GhostEditorMainThread;
#[cfg(not(target_os = "windows"))]
type MainThreadState = ();

pub struct GhostAgentHostPlugin;

const CAPTURE_CAPACITY_FRAMES: usize = 1_152_000;

pub struct GhostShared {
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    graph_control: Arc<AtomicGraphControl>,
    host: HostSharedHandle<'static>,
    commands: Arc<Mutex<Vec<MainThreadCommand>>>,
    parameter_control: Arc<ghost_host::RealtimeParameterControl>,
}

impl GhostShared {
    fn new(host: HostSharedHandle<'_>) -> Self {
        // SAFETY: GhostShared is destroyed with the plugin instance and never invokes the handle
        // during or after its own destruction.
        let host = unsafe { host.with_arbitrary_lifetime() };
        Self {
            daw: Arc::new(AtomicDawState::default()),
            capture: Arc::new(RealtimeCaptureBuffer::new(CAPTURE_CAPACITY_FRAMES)),
            graph_control: Arc::new(AtomicGraphControl::default()),
            host,
            commands: Arc::new(Mutex::new(Vec::new())),
            parameter_control: Arc::new(ghost_host::RealtimeParameterControl::new()),
        }
    }
}

enum MainThreadCommand {
    ShowChildGui(String),
    HideChildGui(String),
    SyncChildStates,
    MarkDirty,
}

struct ClapHostControl {
    host: HostSharedHandle<'static>,
    commands: Arc<Mutex<Vec<MainThreadCommand>>>,
    parameter_control: Arc<ghost_host::RealtimeParameterControl>,
}

struct OuterNestedHostBridge {
    host: HostSharedHandle<'static>,
}

impl ghost_host::NestedHostBridge for OuterNestedHostBridge {
    fn request_restart(&self) {
        self.host.request_restart();
    }

    fn request_process(&self) {
        self.host.request_process();
    }

    fn request_params_flush(&self) {
        if let Some(params) = self.host.get_extension::<HostParams>() {
            params.request_flush(&self.host);
        } else {
            self.host.request_process();
        }
    }

    fn request_main_thread(&self) {
        self.host.request_callback();
    }
}

impl ghost_ui::HostControl for ClapHostControl {
    fn request_graph_restart(&self) {
        self.host.request_restart();
    }

    fn request_process(&self) {
        self.host.request_process();
    }

    fn mark_project_dirty(&self) {
        if let Ok(mut commands) = self.commands.lock() {
            commands.push(MainThreadCommand::MarkDirty);
            self.host.request_callback();
        }
    }

    fn queue_parameter_patch(
        &self,
        patch: &ghost_host::CompiledParameterPatch,
    ) -> Result<(), String> {
        self.parameter_control
            .enqueue_patch(patch)
            .map_err(|error| format!("{error:?}"))
    }

    fn drain_parameter_acknowledgements(&self, output: &mut Vec<ghost_host::ParameterAck>) {
        self.parameter_control.drain_acknowledgements(output);
    }

    fn parameter_transaction_complete(&self, _transaction_id: u64) {
        if let Ok(mut commands) = self.commands.lock() {
            commands.push(MainThreadCommand::SyncChildStates);
            commands.push(MainThreadCommand::MarkDirty);
            self.host.request_callback();
        }
    }

    fn show_child_gui(&self, node_id: &str) {
        if let Ok(mut commands) = self.commands.lock() {
            commands.push(MainThreadCommand::ShowChildGui(node_id.into()));
            self.host.request_callback();
        }
    }

    fn hide_child_gui(&self, node_id: &str) {
        if let Ok(mut commands) = self.commands.lock() {
            commands.push(MainThreadCommand::HideChildGui(node_id.into()));
            self.host.request_callback();
        }
    }
}

impl PluginShared<'_> for GhostShared {}

impl Plugin for GhostAgentHostPlugin {
    type AudioProcessor<'a> = GhostAudioProcessor;
    type Shared<'a> = GhostShared;
    type MainThread<'a> = MainThreadState;

    fn declare_extensions(
        builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        #[cfg(target_os = "windows")]
        {
            builder.register::<PluginAudioPorts>();
            builder.register::<clack_extensions::gui::PluginGui>();
            builder.register::<clack_extensions::state::PluginState>();
            builder.register::<clack_extensions::latency::PluginLatency>();
            builder.register::<PluginParams>();
        }
        #[cfg(not(target_os = "windows"))]
        builder.register::<PluginAudioPorts>();
    }
}

impl DefaultPluginFactory for GhostAgentHostPlugin {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new("ai.konko.ghost-agent-host", "Ghost Agent Host")
            .with_vendor("Konko")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_description("AI-assisted audio analysis and child-plugin host")
            .with_features([AUDIO_EFFECT, ANALYZER, STEREO])
    }

    fn new_shared(host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(GhostShared::new(host))
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        #[cfg(target_os = "windows")]
        return Ok(MainThreadState::new(shared, host));
        #[cfg(not(target_os = "windows"))]
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
impl PluginAudioPortsImpl for () {
    fn count(&mut self, _is_input: bool) -> u32 {
        1
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        write_stereo_port(index, is_input, writer);
    }
}

fn write_stereo_port(index: u32, is_input: bool, writer: &mut AudioPortInfoWriter<'_>) {
    if index == 0 {
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

pub struct GhostAudioProcessor {
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    children: Vec<(String, u64, u64, NativeClapAudio)>,
    graph_control: Arc<AtomicGraphControl>,
    parameter_control: Arc<ghost_host::RealtimeParameterControl>,
    active_graph_revision: u64,
    capture_left: Vec<f32>,
    capture_right: Vec<f32>,
    selected_left: Vec<f32>,
    selected_right: Vec<f32>,
}

impl<'a> PluginAudioProcessor<'a, GhostShared, MainThreadState> for GhostAudioProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut MainThreadState,
        shared: &'a GhostShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        shared.daw.set_audio_configuration(
            audio_config.sample_rate,
            audio_config.min_frames_count,
            audio_config.max_frames_count,
        );
        let process_config = ProcessConfig {
            sample_rate: audio_config.sample_rate.round() as u32,
            maximum_frames: audio_config.max_frames_count as usize,
            channels: 2,
        };
        let (active_graph_revision, children) = _main_thread.activate_children(process_config);
        Ok(Self {
            daw: Arc::clone(&shared.daw),
            capture: Arc::clone(&shared.capture),
            children,
            graph_control: Arc::clone(&shared.graph_control),
            parameter_control: Arc::clone(&shared.parameter_control),
            active_graph_revision,
            capture_left: vec![0.0; audio_config.max_frames_count as usize],
            capture_right: vec![0.0; audio_config.max_frames_count as usize],
            selected_left: vec![0.0; audio_config.max_frames_count as usize],
            selected_right: vec![0.0; audio_config.max_frames_count as usize],
        })
    }

    fn process(
        &mut self,
        process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // CLAP defines process.transport as the transport state at sample zero. Transport events
        // inside the block describe later sample-accurate changes and must not replace this value.
        let block_transport = process.transport;
        self.daw
            .publish_transport(transport_snapshot(process.steady_time, block_transport));
        self.apply_parameter_commands();
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
                    let frames = left_in.len().min(self.capture_left.len());
                    self.capture_left[..frames].copy_from_slice(&left_in[..frames]);
                    self.capture_right[..frames].copy_from_slice(&right_in[..frames]);
                    left_out.copy_from_slice(left_in);
                    right_out.copy_from_slice(right_in);
                    let actual_tap = self.process_children(
                        left_out,
                        right_out,
                        process.steady_time,
                        block_transport,
                    )?;
                    self.capture.push_stereo_for_tap(
                        &self.capture_left[..frames],
                        &self.capture_right[..frames],
                        &self.selected_left[..frames],
                        &self.selected_right[..frames],
                        actual_tap,
                    );
                }
                (Some(ChannelPair::InPlace(left)), Some(ChannelPair::InPlace(right))) => {
                    let frames = left.len().min(self.capture_left.len());
                    self.capture_left[..frames].copy_from_slice(&left[..frames]);
                    self.capture_right[..frames].copy_from_slice(&right[..frames]);
                    let actual_tap =
                        self.process_children(left, right, process.steady_time, block_transport)?;
                    self.capture.push_stereo_for_tap(
                        &self.capture_left[..frames],
                        &self.capture_right[..frames],
                        &self.selected_left[..frames],
                        &self.selected_right[..frames],
                        actual_tap,
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

    fn deactivate(self, main_thread: &mut MainThreadState) {
        self.daw.clear_audio_configuration();
        self.capture.cancel();
        main_thread.deactivate_children(self.children);
    }
}

impl GhostAudioProcessor {
    fn apply_parameter_commands(&mut self) {
        while let Some(transaction) = self.parameter_control.pop_transaction() {
            let rejection = if transaction.expected_graph_revision != self.active_graph_revision {
                Some((0, ghost_host::ParameterAckStatus::GraphRevisionMismatch))
            } else {
                transaction
                    .changes
                    .iter()
                    .enumerate()
                    .find_map(|(index, change)| {
                        let child = self
                            .children
                            .iter()
                            .find(|(node_id, _, _, _)| node_id == &change.target_node_id)
                            .map(|(_, _, _, child)| child);
                        match child {
                            None => Some((index, ghost_host::ParameterAckStatus::NodeUnavailable)),
                            Some(child) => child
                                .can_set_parameter_plain(&change.parameter_id, change.plain_value)
                                .err()
                                .map(|_| {
                                    (index, ghost_host::ParameterAckStatus::ParameterRejected)
                                }),
                        }
                    })
            };
            if let Some((rejected_index, status)) = rejection {
                for (index, change) in transaction.changes.into_iter().enumerate() {
                    self.parameter_control
                        .acknowledge(ghost_host::ParameterAck {
                            transaction_id: transaction.transaction_id,
                            node_id: change.target_node_id,
                            parameter_id: change.parameter_id,
                            value: change.plain_value,
                            previous_value: None,
                            status: if index == rejected_index {
                                status
                            } else {
                                ghost_host::ParameterAckStatus::TransactionRejected
                            },
                        });
                }
                continue;
            }
            for change in transaction.changes {
                let (status, previous_value) = if let Some((_, _, _, child)) = self
                    .children
                    .iter_mut()
                    .find(|(node_id, _, _, _)| node_id == &change.target_node_id)
                {
                    match child.set_parameter_plain(&change.parameter_id, change.plain_value) {
                        Ok(previous) => (ghost_host::ParameterAckStatus::Applied, Some(previous)),
                        Err(_) => (ghost_host::ParameterAckStatus::ParameterRejected, None),
                    }
                } else {
                    (ghost_host::ParameterAckStatus::NodeUnavailable, None)
                };
                self.parameter_control
                    .acknowledge(ghost_host::ParameterAck {
                        transaction_id: transaction.transaction_id,
                        node_id: change.target_node_id,
                        parameter_id: change.parameter_id,
                        value: change.plain_value,
                        previous_value,
                        status,
                    });
            }
        }
    }

    fn process_children(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        steady_time: Option<u64>,
        transport: Option<&TransportEvent>,
    ) -> Result<u64, PluginError> {
        let frames = left.len().min(right.len());
        let selected_tap = self.capture.tap_key();
        if selected_tap == capture_tap_key("input") {
            self.selected_left[..frames].copy_from_slice(&left[..frames]);
            self.selected_right[..frames].copy_from_slice(&right[..frames]);
        }
        let mut matched = selected_tap == capture_tap_key("input");
        for (_, post_key, bypass_bit, child) in &mut self.children {
            if !self.graph_control.is_bypassed(*bypass_bit) {
                let mut channels: [&mut [f32]; 2] = [&mut left[..frames], &mut right[..frames]];
                child
                    .process_with_transport(
                        &mut AudioBlock {
                            channels: &mut channels,
                            frames,
                        },
                        steady_time,
                        transport,
                    )
                    .map_err(|_| PluginError::Message("Native child processing failed"))?;
            }
            if selected_tap == *post_key {
                self.selected_left[..frames].copy_from_slice(&left[..frames]);
                self.selected_right[..frames].copy_from_slice(&right[..frames]);
                matched = true;
            }
        }
        let actual_tap = if selected_tap == capture_tap_key("output") || !matched {
            self.selected_left[..frames].copy_from_slice(&left[..frames]);
            self.selected_right[..frames].copy_from_slice(&right[..frames]);
            capture_tap_key("output")
        } else {
            selected_tap
        };
        Ok(actual_tap)
    }
}

#[cfg(target_os = "windows")]
impl PluginAudioProcessorParams for GhostAudioProcessor {
    fn flush(
        &mut self,
        _input_parameter_changes: &clack_common::events::io::InputEvents,
        _output_parameter_changes: &mut clack_common::events::io::OutputEvents,
    ) {
        self.apply_parameter_commands();
        for (_, _, _, child) in &mut self.children {
            let _ = child.flush_parameter_events();
        }
    }
}

fn passthrough(pair: ChannelPair<'_, f32>) {
    match pair {
        ChannelPair::InputOnly(_) | ChannelPair::InPlace(_) => {}
        ChannelPair::OutputOnly(output) => output.fill(0.0),
        ChannelPair::InputOutput(input, output) => output.copy_from_slice(input),
    }
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

clack_export_entry!(SinglePluginEntry<GhostAgentHostPlugin>);
