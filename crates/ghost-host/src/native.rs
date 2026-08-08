//! Production CLAP loading, public-parameter inspection, state, and block processing adapter.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfoBuffer, PluginAudioPorts,
};
#[cfg(target_os = "windows")]
use clack_extensions::gui::{GuiApiType, GuiConfiguration, PluginGui, Window};
use clack_extensions::latency::PluginLatency;
use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_extensions::state::PluginState;
use clack_extensions::timer::PluginTimer;
use clack_host::events::event_types::ParamValueEvent;
use clack_host::prelude::*;
use clack_host::utils::Cookie;
use ghost_core::{ParameterDescriptor, ProcessorDescriptor};

use crate::native_host::{
    with_audio_thread_scope, NativeHost, NativeHostMain, NativeHostShared, NestedHostBridge,
    NestedHostEvent, NoopNestedHostBridge,
};
use crate::{
    AudioBlock, ChildError, ChildStateBlob, HostError, ParentWindow, PluginDescriptorRecord,
    ProcessConfig,
};

#[derive(Debug, Clone)]
struct ChildAudioPort {
    name: String,
    channel_count: usize,
    is_main: bool,
}

#[derive(Debug, Clone)]
struct ChildAudioTopology {
    inputs: Vec<ChildAudioPort>,
    outputs: Vec<ChildAudioPort>,
    main_input: usize,
    main_output: usize,
}

impl ChildAudioTopology {
    fn total_input_channels(&self) -> usize {
        self.inputs.iter().map(|port| port.channel_count).sum()
    }

    fn total_output_channels(&self) -> usize {
        self.outputs.iter().map(|port| port.channel_count).sum()
    }
}

/// Inspects every descriptor in a CLAP file and retrieves its public parameter manifest without
/// activating audio processing.
pub fn inspect_clap_file(path: impl AsRef<Path>) -> Result<Vec<PluginDescriptorRecord>, HostError> {
    let path = path.as_ref();
    let mut records = crate::clack_runtime::scan_clap_file(path)?;
    for record in &mut records {
        record.public_parameters = inspect_parameters(path, &record.id)?;
    }
    Ok(records)
}

fn inspect_parameters(path: &Path, plugin_id: &str) -> Result<Vec<ParameterDescriptor>, HostError> {
    let entry =
        unsafe { PluginEntry::load(path) }.map_err(|error| HostError::Scan(error.to_string()))?;
    let host_info = host_info()?;
    let id = CString::new(plugin_id).map_err(|error| HostError::Scan(error.to_string()))?;
    let bridge: Arc<dyn NestedHostBridge> = Arc::new(NoopNestedHostBridge);
    let mut instance = PluginInstance::<NativeHost>::new(
        move |_| NativeHostShared::new(bridge),
        |shared| NativeHostMain::new(shared),
        &entry,
        &id,
        &host_info,
    )
    .map_err(|error| HostError::Scan(error.to_string()))?;
    parameter_manifest(&mut instance)
}

fn parameter_manifest(
    instance: &mut PluginInstance<NativeHost>,
) -> Result<Vec<ParameterDescriptor>, HostError> {
    let mut plugin = instance.plugin_handle();
    let Some(params) = plugin.get_extension::<PluginParams>() else {
        return Ok(Vec::new());
    };
    let count = params.count(&mut plugin);
    let mut manifest = Vec::with_capacity(count as usize);
    for index in 0..count {
        let mut buffer = ParamInfoBuffer::new();
        let Some(info) = params.get_info(&mut plugin, index, &mut buffer) else {
            continue;
        };
        let mut display = [0_u8; 256];
        let default_text = params
            .value_to_text(&mut plugin, info.id, info.default_value, &mut display)
            .ok()
            .map(|text| {
                String::from_utf8_lossy(text)
                    .trim_matches('\0')
                    .trim()
                    .to_owned()
            });
        let unit = default_text.as_deref().and_then(infer_display_unit);
        let mut labels = BTreeMap::new();
        if info.flags.contains(ParamInfoFlags::IS_STEPPED) {
            let start = info.min_value.ceil() as i64;
            let end = info.max_value.floor() as i64;
            if end >= start && end - start <= 64 {
                for raw in start..=end {
                    if let Ok(text) =
                        params.value_to_text(&mut plugin, info.id, raw as f64, &mut display)
                    {
                        let label = String::from_utf8_lossy(text)
                            .trim_matches('\0')
                            .trim()
                            .to_owned();
                        if !label.is_empty() {
                            labels.insert(label, raw as f64);
                        }
                    }
                }
            }
        }
        manifest.push(ParameterDescriptor {
            stable_id: info.id.get().to_string(),
            name: String::from_utf8_lossy(info.name).into_owned(),
            module: (!info.module.is_empty())
                .then(|| String::from_utf8_lossy(info.module).into_owned()),
            unit,
            minimum: info.min_value,
            maximum: info.max_value,
            default: info.default_value,
            stepped: info.flags.contains(ParamInfoFlags::IS_STEPPED),
            read_only: info.flags.contains(ParamInfoFlags::IS_READONLY),
            labels,
        });
    }
    Ok(manifest)
}

fn audio_topology(instance: &mut PluginInstance<NativeHost>) -> Result<ChildAudioTopology, ChildError> {
    let mut plugin = instance.plugin_handle();
    let ports = plugin
        .get_extension::<PluginAudioPorts>()
        .ok_or_else(|| ChildError::Unsupported("clap.audio-ports".into()))?;
    let inspect = |plugin: &mut PluginMainThreadHandle<'_>, is_input: bool| {
        let count = ports.count(plugin, is_input);
        let mut result = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut buffer = AudioPortInfoBuffer::new();
            let info = ports.get(plugin, index, is_input, &mut buffer).ok_or_else(|| {
                ChildError::Failed(format!(
                    "child failed to describe {} audio port {index}",
                    if is_input { "input" } else { "output" }
                ))
            })?;
            if info.channel_count == 0 {
                return Err(ChildError::Failed(format!(
                    "child {} audio port {index} has zero channels",
                    if is_input { "input" } else { "output" }
                )));
            }
            result.push(ChildAudioPort {
                name: String::from_utf8_lossy(info.name).into_owned(),
                channel_count: info.channel_count as usize,
                is_main: info.flags.contains(AudioPortFlags::IS_MAIN),
            });
        }
        Ok::<_, ChildError>(result)
    };
    let inputs = inspect(&mut plugin, true)?;
    let outputs = inspect(&mut plugin, false)?;
    if inputs.is_empty() || outputs.is_empty() {
        return Err(ChildError::Unsupported(
            "Ghost graph currently requires an audio input and output".into(),
        ));
    }
    let main_input = inputs.iter().position(|port| port.is_main).unwrap_or(0);
    let main_output = outputs.iter().position(|port| port.is_main).unwrap_or(0);
    Ok(ChildAudioTopology {
        inputs,
        outputs,
        main_input,
        main_output,
    })
}

fn infer_display_unit(text: &str) -> Option<String> {
    let unit = text
        .trim()
        .trim_start_matches(|character: char| {
            character.is_ascii_digit()
                || matches!(character, '+' | '-' | '.' | ',' | 'e' | 'E')
                || character.is_whitespace()
        })
        .trim();
    (!unit.is_empty() && unit.len() <= 16).then(|| unit.to_ascii_lowercase())
}

/// Main-thread half of a native child. It never crosses onto the audio thread.
pub struct NativeClapMain {
    instance: PluginInstance<NativeHost>,
    descriptor: ProcessorDescriptor,
    gui_created: bool,
    gui_mode: Option<ChildGuiMode>,
    gui_session: ChildWindowSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildGuiMode {
    PluginFloating,
    HostedDetached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildWindowSession {
    Closed,
    PluginFloating { visible: bool },
    HostedDetached { visible: bool },
    CloseRequested,
    Destroyed,
}

impl NativeClapMain {
    pub fn can_set_parameter_plain(
        &self,
        parameter_id: &str,
        value: f64,
    ) -> Result<(), ChildError> {
        validate_parameter(&self.descriptor, parameter_id, value)
    }

    /// Flushes a parameter change while the child is inactive. This is the CLAP-defined path for
    /// editor changes when no process block is available.
    pub fn set_parameter_plain_inactive(
        &mut self,
        parameter_id: &str,
        value: f64,
    ) -> Result<f64, ChildError> {
        let raw_id = parameter_id
            .parse::<u32>()
            .ok()
            .and_then(ClapId::from_raw)
            .ok_or(ChildError::UnknownRealtimeParameter)?;
        self.can_set_parameter_plain(parameter_id, value)?;
        let parameter = self
            .descriptor
            .parameters
            .iter()
            .find(|parameter| parameter.stable_id == parameter_id)
            .ok_or(ChildError::UnknownRealtimeParameter)?;
        let (params, previous) = {
            let mut plugin = self.instance.plugin_handle();
            let params = plugin
                .get_extension::<PluginParams>()
                .ok_or_else(|| ChildError::Unsupported("clap.params".into()))?;
            let previous = params
                .get_value(&mut plugin, raw_id)
                .unwrap_or(parameter.default);
            (params, previous)
        };

        let mut input = EventBuffer::with_capacity(1);
        input.push(&ParamValueEvent::new(
            0,
            raw_id,
            Pckn::match_all(),
            value,
            Cookie::empty(),
        ));
        let mut output = EventBuffer::with_capacity(16);
        let input_events = input.as_input();
        let mut output_events = output.as_output();
        let mut inactive = self
            .instance
            .inactive_plugin_handle()
            .ok_or(ChildError::NotActive)?;
        params.flush(&mut inactive, &input_events, &mut output_events);
        Ok(previous)
    }

    pub fn open(path: impl AsRef<Path>, plugin_id: &str) -> Result<Self, ChildError> {
        Self::open_with_bridge(path, plugin_id, Arc::new(NoopNestedHostBridge))
    }

    pub fn open_with_bridge(
        path: impl AsRef<Path>,
        plugin_id: &str,
        bridge: Arc<dyn NestedHostBridge>,
    ) -> Result<Self, ChildError> {
        let path = path.as_ref();
        let entry = unsafe { PluginEntry::load(path) }
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        let host_info = host_info().map_err(|error| ChildError::Failed(error.to_string()))?;
        let id = CString::new(plugin_id).map_err(|error| ChildError::Failed(error.to_string()))?;
        let mut instance = PluginInstance::<NativeHost>::new(
            move |_| NativeHostShared::new(bridge),
            |shared| NativeHostMain::new(shared),
            &entry,
            &id,
            &host_info,
        )
        .map_err(|error| ChildError::Failed(error.to_string()))?;
        let parameters = parameter_manifest(&mut instance)
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        let record = crate::clack_runtime::scan_clap_file(path)
            .map_err(|error| ChildError::Failed(error.to_string()))?
            .into_iter()
            .find(|record| record.id == plugin_id)
            .ok_or_else(|| ChildError::Failed(format!("plugin `{plugin_id}` disappeared")))?;
        Ok(Self {
            instance,
            descriptor: ProcessorDescriptor {
                stable_id: record.id,
                name: record.name,
                vendor: record.vendor,
                version: record.version,
                capabilities: Vec::new(),
                parameters,
            },
            gui_created: false,
            gui_mode: None,
            gui_session: ChildWindowSession::Closed,
        })
    }

    pub fn descriptor(&self) -> &ProcessorDescriptor {
        &self.descriptor
    }

    pub fn latency_samples(&mut self) -> u32 {
        let mut plugin = self.instance.plugin_handle();
        plugin
            .get_extension::<PluginLatency>()
            .map_or(0, |latency| latency.get(&mut plugin))
    }

    pub fn parameter_values(&mut self) -> BTreeMap<String, f64> {
        let mut plugin = self.instance.plugin_handle();
        plugin
            .get_extension::<PluginParams>()
            .map(|params| {
                self.descriptor
                    .parameters
                    .iter()
                    .filter_map(|parameter| {
                        let id = parameter
                            .stable_id
                            .parse::<u32>()
                            .ok()
                            .and_then(ClapId::from_raw)?;
                        params
                            .get_value(&mut plugin, id)
                            .map(|value| (parameter.stable_id.clone(), value))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn refresh_parameter_manifest(&mut self) -> Result<(), ChildError> {
        self.descriptor.parameters = parameter_manifest(&mut self.instance)
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        Ok(())
    }

    pub fn gui_session(&self) -> ChildWindowSession {
        self.gui_session
    }

    pub fn activate(&mut self, config: ProcessConfig) -> Result<NativeClapAudio, ChildError> {
        if config.channels == 0 || config.maximum_frames == 0 || config.sample_rate == 0 {
            return Err(ChildError::Failed(
                "invalid native child configuration".into(),
            ));
        }
        let topology = audio_topology(&mut self.instance)?;
        let current_values = self.parameter_values();
        let stopped = self
            .instance
            .activate(
                |_, _| (),
                PluginAudioConfiguration {
                    sample_rate: f64::from(config.sample_rate),
                    min_frames_count: 1,
                    max_frames_count: config.maximum_frames as u32,
                },
            )
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        let mut processor: PluginAudioProcessor<NativeHost> = stopped.into();
        with_audio_thread_scope(|| processor.ensure_processing_started())
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        let input_scratch = topology
            .inputs
            .iter()
            .map(|port| vec![vec![0.0; config.maximum_frames]; port.channel_count])
            .collect();
        let output_scratch = topology
            .outputs
            .iter()
            .map(|port| vec![vec![0.0; config.maximum_frames]; port.channel_count])
            .collect();
        Ok(NativeClapAudio {
            processor: Some(processor),
            descriptor: self.descriptor.clone(),
            config,
            input_ports: AudioPorts::with_capacity(
                topology.total_input_channels(),
                topology.inputs.len(),
            ),
            output_ports: AudioPorts::with_capacity(
                topology.total_output_channels(),
                topology.outputs.len(),
            ),
            topology,
            input_scratch,
            output_scratch,
            output_events: EventBuffer::with_capacity(1024),
            input_events: EventBuffer::with_capacity(256),
            current_values,
        })
    }

    pub fn deactivate(&mut self, mut audio: NativeClapAudio) {
        if let Some(processor) = audio.processor.take() {
            let stopped = with_audio_thread_scope(|| processor.into_stopped());
            self.instance.deactivate(stopped);
        }
    }

    pub fn save_state(&mut self) -> Result<ChildStateBlob, ChildError> {
        let mut plugin = self.instance.plugin_handle();
        let state = plugin
            .get_extension::<PluginState>()
            .ok_or_else(|| ChildError::Unsupported("clap.state".into()))?;
        let mut bytes = Vec::new();
        state
            .save(&mut plugin, &mut bytes)
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        Ok(ChildStateBlob {
            format: "clap.state/1".into(),
            bytes,
        })
    }

    pub fn load_state(&mut self, state_blob: &ChildStateBlob) -> Result<(), ChildError> {
        if state_blob.format != "clap.state/1" {
            return Err(ChildError::Unsupported(state_blob.format.clone()));
        }
        let mut plugin = self.instance.plugin_handle();
        let state = plugin
            .get_extension::<PluginState>()
            .ok_or_else(|| ChildError::Unsupported("clap.state".into()))?;
        state
            .load(&mut plugin, &mut state_blob.bytes.as_slice())
            .map_err(|error| ChildError::Failed(error.to_string()))
    }

    pub fn gui_supported(&mut self) -> bool {
        #[cfg(target_os = "windows")]
        {
            let mut plugin = self.instance.plugin_handle();
            let Some(gui) = plugin.get_extension::<PluginGui>() else {
                return false;
            };
            [true, false].into_iter().any(|is_floating| {
                gui.is_api_supported(
                    &mut plugin,
                    GuiConfiguration {
                        api_type: GuiApiType::WIN32,
                        is_floating,
                    },
                )
            })
        }
        #[cfg(not(target_os = "windows"))]
        false
    }

    /// Creates and parents the child's native editor. The caller must invoke this on the same
    /// main thread that owns the child and keep `parent` alive until `close_gui`.
    pub fn open_gui(&mut self, parent: ParentWindow) -> Result<(), ChildError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = parent;
            Err(ChildError::Unsupported(
                "embedded child GUI requires Windows".into(),
            ))
        }
        #[cfg(target_os = "windows")]
        {
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin
                .get_extension::<PluginGui>()
                .ok_or_else(|| ChildError::Unsupported("clap.gui".into()))?;
            let configuration = GuiConfiguration {
                api_type: GuiApiType::WIN32,
                is_floating: false,
            };
            if !self.gui_created {
                gui.create(&mut plugin, configuration)
                    .map_err(|error| ChildError::Failed(error.to_string()))?;
                self.gui_created = true;
                self.gui_mode = Some(ChildGuiMode::HostedDetached);
                self.gui_session = ChildWindowSession::HostedDetached { visible: false };
                // SAFETY: ParentWindow is supplied by the outer Win32 main-thread adapter and
                // remains alive until the child editor is closed.
                unsafe {
                    gui.set_parent(
                        &mut plugin,
                        Window::from_win32_hwnd(parent.raw as windows_sys::Win32::Foundation::HWND),
                    )
                }
                .map_err(|error| ChildError::Failed(error.to_string()))?;
            }
            gui.show(&mut plugin)
                .map_err(|error| ChildError::Failed(error.to_string()))?;
            self.gui_session = ChildWindowSession::HostedDetached { visible: true };
            Ok(())
        }
    }

    pub fn preferred_gui_mode(&mut self) -> Option<ChildGuiMode> {
        #[cfg(target_os = "windows")]
        {
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin.get_extension::<PluginGui>()?;
            if gui.is_api_supported(
                &mut plugin,
                GuiConfiguration {
                    api_type: GuiApiType::WIN32,
                    is_floating: true,
                },
            ) {
                Some(ChildGuiMode::PluginFloating)
            } else if gui.is_api_supported(
                &mut plugin,
                GuiConfiguration {
                    api_type: GuiApiType::WIN32,
                    is_floating: false,
                },
            ) {
                Some(ChildGuiMode::HostedDetached)
            } else {
                None
            }
        }
        #[cfg(not(target_os = "windows"))]
        None
    }

    pub fn open_floating_gui(
        &mut self,
        transient: ParentWindow,
        title: &str,
    ) -> Result<(), ChildError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (transient, title);
            Err(ChildError::Unsupported(
                "floating child GUI requires Windows".into(),
            ))
        }
        #[cfg(target_os = "windows")]
        {
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin
                .get_extension::<PluginGui>()
                .ok_or_else(|| ChildError::Unsupported("clap.gui".into()))?;
            let configuration = GuiConfiguration {
                api_type: GuiApiType::WIN32,
                is_floating: true,
            };
            if !self.gui_created {
                gui.create(&mut plugin, configuration)
                    .map_err(|error| ChildError::Failed(error.to_string()))?;
                self.gui_created = true;
                self.gui_mode = Some(ChildGuiMode::PluginFloating);
                self.gui_session = ChildWindowSession::PluginFloating { visible: false };
                // SAFETY: the supplied DAW owner window is valid for this main-thread call.
                unsafe {
                    gui.set_transient(
                        &mut plugin,
                        Window::from_win32_hwnd(
                            transient.raw as windows_sys::Win32::Foundation::HWND,
                        ),
                    )
                }
                .map_err(|error| ChildError::Failed(error.to_string()))?;
                if let Ok(title) = CString::new(title) {
                    gui.suggest_title(&mut plugin, &title);
                }
            } else if self.gui_mode != Some(ChildGuiMode::PluginFloating) {
                return Err(ChildError::Failed(
                    "child GUI is already created in another mode".into(),
                ));
            }
            gui.show(&mut plugin)
                .map_err(|error| ChildError::Failed(error.to_string()))?;
            self.gui_session = ChildWindowSession::PluginFloating { visible: true };
            Ok(())
        }
    }

    pub fn gui_size(&mut self) -> Option<clack_extensions::gui::GuiSize> {
        #[cfg(target_os = "windows")]
        {
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin.get_extension::<PluginGui>()?;
            gui.get_size(&mut plugin)
        }
        #[cfg(not(target_os = "windows"))]
        None
    }

    pub fn set_gui_visible(&mut self, visible: bool) -> Result<(), ChildError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = visible;
            Err(ChildError::Unsupported(
                "embedded child GUI requires Windows".into(),
            ))
        }
        #[cfg(target_os = "windows")]
        {
            if !self.gui_created {
                return Err(ChildError::Failed("child GUI has not been opened".into()));
            }
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin
                .get_extension::<PluginGui>()
                .ok_or_else(|| ChildError::Unsupported("clap.gui".into()))?;
            if visible {
                gui.show(&mut plugin)
            } else {
                gui.hide(&mut plugin)
            }
            .map_err(|error| ChildError::Failed(error.to_string()))?;
            self.gui_session = match self.gui_mode {
                Some(ChildGuiMode::PluginFloating) => {
                    ChildWindowSession::PluginFloating { visible }
                }
                Some(ChildGuiMode::HostedDetached) => {
                    ChildWindowSession::HostedDetached { visible }
                }
                None => ChildWindowSession::Closed,
            };
            Ok(())
        }
    }

    pub fn close_gui(&mut self) {
        #[cfg(target_os = "windows")]
        if self.gui_created {
            let mut plugin = self.instance.plugin_handle();
            if let Some(gui) = plugin.get_extension::<PluginGui>() {
                gui.destroy(&mut plugin);
            }
            self.gui_created = false;
            self.gui_mode = None;
            self.gui_session = ChildWindowSession::Closed;
        }
    }

    pub fn acknowledge_gui_closed(&mut self, was_destroyed: bool) {
        if was_destroyed {
            #[cfg(target_os = "windows")]
            if self.gui_created {
                let mut plugin = self.instance.plugin_handle();
                if let Some(gui) = plugin.get_extension::<PluginGui>() {
                    gui.destroy(&mut plugin);
                }
                self.gui_created = false;
                self.gui_mode = None;
            }
            self.gui_session = ChildWindowSession::Destroyed;
        } else {
            self.gui_session = ChildWindowSession::CloseRequested;
        }
    }

    pub fn set_gui_size(&mut self, size: clack_extensions::gui::GuiSize) -> Result<(), ChildError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = size;
            Err(ChildError::Unsupported("child GUI requires Windows".into()))
        }
        #[cfg(target_os = "windows")]
        {
            if !self.gui_created {
                return Err(ChildError::Failed("child GUI has not been opened".into()));
            }
            let mut plugin = self.instance.plugin_handle();
            let gui = plugin
                .get_extension::<PluginGui>()
                .ok_or_else(|| ChildError::Unsupported("clap.gui".into()))?;
            gui.set_size(&mut plugin, size)
                .map_err(|error| ChildError::Failed(error.to_string()))
        }
    }

    /// Services child core callbacks and timers, then drains all bounded child-host notifications.
    pub fn service_host_callbacks(&mut self) -> Vec<NestedHostEvent> {
        if self
            .instance
            .access_shared_handler(NativeHostShared::take_callback_request)
        {
            self.instance.call_on_main_thread_callback();
        }
        let mut due = Vec::new();
        self.instance
            .access_handler_mut(|handler| handler.due_timers(Instant::now(), &mut due));
        if !due.is_empty() {
            let mut plugin = self.instance.plugin_handle();
            if let Some(timer) = plugin.get_extension::<PluginTimer>() {
                for timer_id in due {
                    timer.on_timer(&mut plugin, timer_id);
                }
            }
        }
        let mut events = Vec::new();
        self.instance
            .access_shared_handler(|shared| shared.drain_events(&mut events));
        events
    }
}

impl Drop for NativeClapMain {
    fn drop(&mut self) {
        self.close_gui();
    }
}

/// Sendable active half. All buffers and event storage are preallocated during activation.
pub struct NativeClapAudio {
    processor: Option<PluginAudioProcessor<NativeHost>>,
    descriptor: ProcessorDescriptor,
    config: ProcessConfig,
    topology: ChildAudioTopology,
    input_ports: AudioPorts,
    output_ports: AudioPorts,
    input_scratch: Vec<Vec<Vec<f32>>>,
    output_scratch: Vec<Vec<Vec<f32>>>,
    output_events: EventBuffer,
    input_events: EventBuffer,
    current_values: BTreeMap<String, f64>,
}

impl NativeClapAudio {
    pub fn descriptor(&self) -> &ProcessorDescriptor {
        &self.descriptor
    }

    pub fn set_parameter_plain(
        &mut self,
        parameter_id: &str,
        value: f64,
    ) -> Result<f64, ChildError> {
        let raw_id = parameter_id
            .parse::<u32>()
            .ok()
            .and_then(ClapId::from_raw)
            .ok_or(ChildError::UnknownRealtimeParameter)?;
        self.can_set_parameter_plain(parameter_id, value)?;
        let parameter = self
            .descriptor
            .parameters
            .iter()
            .find(|parameter| parameter.stable_id == parameter_id)
            .ok_or(ChildError::UnknownRealtimeParameter)?;
        self.input_events.push(&ParamValueEvent::new(
            0,
            raw_id,
            Pckn::match_all(),
            value,
            Cookie::empty(),
        ));
        let previous = self
            .current_values
            .get_mut(parameter_id)
            .map(|current| std::mem::replace(current, value))
            .unwrap_or(parameter.default);
        Ok(previous)
    }

    pub fn can_set_parameter_plain(
        &self,
        parameter_id: &str,
        value: f64,
    ) -> Result<(), ChildError> {
        validate_parameter(&self.descriptor, parameter_id, value)
    }

    pub fn process(&mut self, block: &mut AudioBlock<'_>) -> Result<(), ChildError> {
        self.process_with_transport(block, None, None)
    }

    /// Delivers pending host parameter events without requiring an audio block. The outer plugin
    /// calls this from its own `params.flush` callback, which CLAP serializes against `process`.
    pub fn flush_parameter_events(&mut self) -> Result<(), ChildError> {
        self.output_events.clear();
        let input_events = self.input_events.as_input();
        let mut output_events = self.output_events.as_output();
        let processor = self.processor.as_mut().ok_or(ChildError::NotActive)?;
        let mut plugin = processor.plugin_handle();
        let params = plugin
            .get_extension::<PluginParams>()
            .ok_or_else(|| ChildError::Unsupported("clap.params".into()))?;
        with_audio_thread_scope(|| {
            params.flush_active(&mut plugin, &input_events, &mut output_events);
        });
        self.consume_parameter_output_events()?;
        self.input_events.clear();
        Ok(())
    }

    pub fn process_with_transport(
        &mut self,
        block: &mut AudioBlock<'_>,
        steady_time: Option<u64>,
        transport: Option<&clack_host::events::event_types::TransportEvent>,
    ) -> Result<(), ChildError> {
        if block.channels.len() != self.config.channels || block.frames > self.config.maximum_frames
        {
            return Err(ChildError::BlockShapeMismatch);
        }

        for port in &mut self.input_scratch {
            for channel in port {
                channel[..block.frames].fill(0.0);
            }
        }
        let main_input = &mut self.input_scratch[self.topology.main_input];
        match main_input.len() {
            0 => return Err(ChildError::BlockShapeMismatch),
            1 => {
                for frame in 0..block.frames {
                    main_input[0][frame] = 0.5 * (block.channels[0][frame] + block.channels[1][frame]);
                }
            }
            _ => {
                main_input[0][..block.frames].copy_from_slice(&block.channels[0][..block.frames]);
                main_input[1][..block.frames].copy_from_slice(&block.channels[1][..block.frames]);
            }
        }
        for port in &mut self.output_scratch {
            for channel in port {
                channel[..block.frames].fill(0.0);
            }
        }

        let input_audio = self.input_ports.with_input_buffers(self.input_scratch.iter_mut().map(
            |port| AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_input_only(
                    port.iter_mut()
                        .map(|channel| InputChannel::variable(&mut channel[..block.frames])),
                ),
            },
        ));
        let mut output_audio = self.output_ports.with_output_buffers(
            self.output_scratch.iter_mut().map(|port| AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_output_only(
                    port.iter_mut().map(|channel| &mut channel[..block.frames]),
                ),
            }),
        );
        self.output_events.clear();
        let input_events = self.input_events.as_input();
        let mut output_events = self.output_events.as_output();
        self.processor
            .as_ref()
            .ok_or(ChildError::NotActive)?
            .access_shared_handler(|shared| {
                let _ = shared.take_flush_request();
            });
        let process_result = with_audio_thread_scope(|| {
            self.processor
                .as_mut()
                .ok_or(ChildError::NotActive)?
                .as_started_mut()
                .map_err(|_| ChildError::ProcessFailed)?
                .process(
                    &input_audio,
                    &mut output_audio,
                    &input_events,
                    &mut output_events,
                    steady_time,
                    transport,
                )
                .map_err(|_| ChildError::ProcessFailed)
        });
        process_result?;
        self.consume_parameter_output_events()?;

        let main_output = &self.output_scratch[self.topology.main_output];
        match main_output.len() {
            0 => return Err(ChildError::BlockShapeMismatch),
            1 => {
                for destination in block.channels.iter_mut().take(2) {
                    destination[..block.frames].copy_from_slice(&main_output[0][..block.frames]);
                }
            }
            _ => {
                block.channels[0][..block.frames].copy_from_slice(&main_output[0][..block.frames]);
                block.channels[1][..block.frames].copy_from_slice(&main_output[1][..block.frames]);
            }
        }
        self.input_events.clear();
        Ok(())
    }

    fn consume_parameter_output_events(&mut self) -> Result<(), ChildError> {
        self.processor
            .as_ref()
            .ok_or(ChildError::NotActive)?
            .access_shared_handler(|shared| {
                for event in self.output_events.iter() {
                    if let Some(parameter) = event.as_event::<ParamValueEvent>() {
                        if let Some(id) = parameter.param_id() {
                            if let Some(descriptor) =
                                self.descriptor.parameters.iter().find(|item| {
                                    item.stable_id.parse::<u32>().ok() == Some(id.get())
                                })
                            {
                                if let Some(current) =
                                    self.current_values.get_mut(&descriptor.stable_id)
                                {
                                    *current = parameter.value();
                                }
                            }
                            shared.parameter_feedback(id.get(), parameter.value());
                        }
                    }
                }
            });
        Ok(())
    }
}

fn validate_parameter(
    descriptor: &ProcessorDescriptor,
    parameter_id: &str,
    value: f64,
) -> Result<(), ChildError> {
    let parameter = descriptor
        .parameters
        .iter()
        .find(|parameter| parameter.stable_id == parameter_id)
        .ok_or(ChildError::UnknownRealtimeParameter)?;
    if parameter.read_only || !(parameter.minimum..=parameter.maximum).contains(&value) {
        return Err(ChildError::InvalidValue(value));
    }
    Ok(())
}

/// Convenience composition used by CLI/offline validation where one thread owns both halves.
pub struct NativeClapSession {
    main: Option<NativeClapMain>,
    audio: Option<NativeClapAudio>,
}

impl NativeClapSession {
    pub fn open(
        path: impl AsRef<Path>,
        plugin_id: &str,
        config: ProcessConfig,
    ) -> Result<Self, ChildError> {
        let mut main = NativeClapMain::open(path, plugin_id)?;
        let audio = main.activate(config)?;
        Ok(Self {
            main: Some(main),
            audio: Some(audio),
        })
    }

    pub fn descriptor(&self) -> &ProcessorDescriptor {
        self.main
            .as_ref()
            .expect("session main is present")
            .descriptor()
    }

    pub fn set_parameter_plain(&mut self, id: &str, value: f64) -> Result<(), ChildError> {
        self.audio
            .as_mut()
            .ok_or(ChildError::NotActive)?
            .set_parameter_plain(id, value)
            .map(|_| ())
    }

    pub fn flush_parameter_events(&mut self) -> Result<(), ChildError> {
        self.audio
            .as_mut()
            .ok_or(ChildError::NotActive)?
            .flush_parameter_events()
    }

    pub fn process(&mut self, block: &mut AudioBlock<'_>) -> Result<(), ChildError> {
        self.audio
            .as_mut()
            .ok_or(ChildError::NotActive)?
            .process(block)
    }

    pub fn save_state(&mut self) -> Result<ChildStateBlob, ChildError> {
        self.main
            .as_mut()
            .ok_or(ChildError::NotActive)?
            .save_state()
    }

    pub fn load_state(&mut self, state: &ChildStateBlob) -> Result<(), ChildError> {
        self.main
            .as_mut()
            .ok_or(ChildError::NotActive)?
            .load_state(state)
    }
}

impl Drop for NativeClapSession {
    fn drop(&mut self) {
        if let (Some(mut main), Some(audio)) = (self.main.take(), self.audio.take()) {
            main.deactivate(audio);
        }
    }
}

fn host_info() -> Result<HostInfo, HostError> {
    HostInfo::new(
        "Ghost Agent Host",
        "Konko",
        "https://github.com/free-audio/clap",
        env!("CARGO_PKG_VERSION"),
    )
    .map_err(|error| HostError::Unavailable(error.to_string()))
}
