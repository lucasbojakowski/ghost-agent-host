use std::path::PathBuf;

use ghost_core::audio::AudioBuffer;
use ghost_mix::MixPlan;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod child;
mod discovery;
mod graph;
mod mapping;
mod mock_dsp;
#[cfg(feature = "clack-runtime")]
mod native;
#[cfg(feature = "clack-runtime")]
mod native_host;
mod parameter_control;
mod smoothing;
mod topology;

pub use child::*;
pub use discovery::*;
pub use graph::*;
pub use mapping::*;
#[cfg(feature = "clack-runtime")]
pub use native::*;
#[cfg(feature = "clack-runtime")]
pub use native_host::{NestedHostBridge, NestedHostEvent, NoopNestedHostBridge};
pub use parameter_control::*;
pub use smoothing::*;
pub use topology::*;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("plugin host backend is unavailable: {0}")]
    Unavailable(String),
    #[error("plugin state error: {0}")]
    State(String),
    #[error("plugin processing error: {0}")]
    Processing(String),
    #[error("plugin scan error: {0}")]
    Scan(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptorRecord {
    pub id: String,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub path: PathBuf,
    #[serde(default)]
    pub public_parameters: Vec<ghost_core::ParameterDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainState {
    pub schema_version: String,
    pub pro_q_state: Vec<u8>,
    pub pro_c_state: Vec<u8>,
    pub accepted_plan: Option<MixPlan>,
}

pub trait HostedChain {
    fn backend_name(&self) -> &'static str;
    fn render(&mut self, source: &AudioBuffer, plan: &MixPlan) -> Result<AudioBuffer, HostError>;
    fn save_state(&self) -> Result<ChainState, HostError>;
    fn load_state(&mut self, state: &ChainState) -> Result<(), HostError>;
}

#[derive(Default)]
pub struct MockFabFilterChain {
    current_plan: Option<MixPlan>,
}

impl HostedChain for MockFabFilterChain {
    fn backend_name(&self) -> &'static str {
        "mock-fabfilter-chain"
    }

    fn render(&mut self, source: &AudioBuffer, plan: &MixPlan) -> Result<AudioBuffer, HostError> {
        self.current_plan = Some(plan.clone());
        Ok(mock_dsp::render_mock_chain(source, plan))
    }

    fn save_state(&self) -> Result<ChainState, HostError> {
        let plan_bytes = serde_json::to_vec(&self.current_plan)
            .map_err(|error| HostError::State(error.to_string()))?;
        Ok(ChainState {
            schema_version: "ghost.chain-state/1".into(),
            pro_q_state: plan_bytes.clone(),
            pro_c_state: plan_bytes,
            accepted_plan: self.current_plan.clone(),
        })
    }

    fn load_state(&mut self, state: &ChainState) -> Result<(), HostError> {
        if state.schema_version != "ghost.chain-state/1" {
            return Err(HostError::State(format!(
                "unsupported chain state {}",
                state.schema_version
            )));
        }
        self.current_plan = state.accepted_plan.clone();
        Ok(())
    }
}

#[cfg(feature = "clack-runtime")]
pub mod clack_runtime {
    use std::path::Path;

    use super::*;
    use clack_extensions::gui::{GuiApiType, GuiConfiguration, PluginGui, Window};
    use clack_extensions::state::PluginState;
    use clack_host::prelude::{HostHandlers, HostInfo, PluginEntry, PluginInstance};

    #[cfg(target_os = "windows")]
    use windows_sys::Win32::Foundation::HWND;
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, FindWindowExW, GetWindowLongPtrW,
        PeekMessageW, TranslateMessage, GWL_STYLE, MSG, PM_REMOVE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    struct ValidationHost;

    impl HostHandlers for ValidationHost {
        type Shared<'a> = ();
        type MainThread<'a> = ();
        type AudioProcessor<'a> = ();
    }

    #[cfg(target_os = "windows")]
    struct ValidationWindow(HWND);

    #[cfg(target_os = "windows")]
    impl ValidationWindow {
        fn create() -> Result<Self, HostError> {
            let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let window_name: Vec<u16> = "Ghost GUI Validator\0".encode_utf16().collect();
            // SAFETY: The class and title are valid null-terminated UTF-16 strings. The system
            // STATIC class permits a process-local hidden validation window.
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    window_name.as_ptr(),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    900,
                    700,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            if hwnd.is_null() {
                return Err(HostError::Scan(std::io::Error::last_os_error().to_string()));
            }
            Ok(Self(hwnd))
        }

        fn plugin_child(&self) -> Option<HWND> {
            // SAFETY: self is a live window; null class/title match the first direct child.
            let child = unsafe {
                FindWindowExW(
                    self.0,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            };
            (!child.is_null()).then_some(child)
        }
    }

    #[cfg(target_os = "windows")]
    fn pump_messages_for(duration: std::time::Duration) {
        let deadline = std::time::Instant::now() + duration;
        while std::time::Instant::now() < deadline {
            // SAFETY: An all-zero MSG is the documented initial state for PeekMessageW output.
            let mut message: MSG = unsafe { std::mem::zeroed() };
            // SAFETY: message points to writable storage for the duration of each Win32 call.
            while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0
            {
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::yield_now();
        }
    }

    #[cfg(target_os = "windows")]
    fn child_is_shown(child: HWND) -> bool {
        // SAFETY: child is a live HWND found under ValidationWindow.
        unsafe { GetWindowLongPtrW(child, GWL_STYLE) as u32 & WS_VISIBLE != 0 }
    }

    #[cfg(target_os = "windows")]
    impl Drop for ValidationWindow {
        fn drop(&mut self) {
            // SAFETY: This HWND was created by ValidationWindow::create and is still live.
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    pub fn scan_clap_file(
        path: impl AsRef<Path>,
    ) -> Result<Vec<PluginDescriptorRecord>, HostError> {
        let path = path.as_ref();
        let entry = unsafe { PluginEntry::load(path) }
            .map_err(|error| HostError::Scan(error.to_string()))?;
        let factory = entry
            .get_plugin_factory()
            .ok_or_else(|| HostError::Scan("CLAP file has no plugin factory".into()))?;
        let mut records = Vec::new();
        for descriptor in factory.plugin_descriptors() {
            let id = descriptor
                .id()
                .and_then(|value| value.to_str().ok())
                .unwrap_or("unknown")
                .to_owned();
            let name = descriptor
                .name()
                .and_then(|value| value.to_str().ok())
                .unwrap_or("Unnamed CLAP plugin")
                .to_owned();
            let vendor = descriptor
                .vendor()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let version = descriptor
                .version()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            records.push(PluginDescriptorRecord {
                id,
                name,
                vendor,
                version,
                path: path.to_path_buf(),
                public_parameters: Vec::new(),
            });
        }
        Ok(records)
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ClapAudioSmokeResult {
        pub first_output: [f32; 2],
        pub state_bytes: usize,
    }

    /// Deterministic native activation/process/state check. This never starts an agent.
    pub fn smoke_test_clap_audio(
        path: impl AsRef<Path>,
        plugin_id: &str,
        initial_state: Option<&[u8]>,
    ) -> Result<ClapAudioSmokeResult, HostError> {
        let entry = unsafe { PluginEntry::load(path.as_ref()) }
            .map_err(|error| HostError::Scan(error.to_string()))?;
        let host_info = HostInfo::new(
            "Ghost CLAP Validator",
            "Konko",
            "https://github.com/free-audio/clap",
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|error| HostError::Scan(error.to_string()))?;
        let plugin_id = std::ffi::CString::new(plugin_id)
            .map_err(|error| HostError::Scan(error.to_string()))?;
        let mut instance =
            PluginInstance::<ValidationHost>::new(|_| (), |_| (), &entry, &plugin_id, &host_info)
                .map_err(|error| HostError::Scan(error.to_string()))?;
        if let Some(initial_state) = initial_state {
            let mut plugin = instance.plugin_handle();
            let state = plugin
                .get_extension::<PluginState>()
                .ok_or_else(|| HostError::State("CLAP plugin does not expose clap.state".into()))?;
            let mut reader = initial_state;
            state
                .load(&mut plugin, &mut reader)
                .map_err(|error| HostError::State(error.to_string()))?;
        }
        let stopped = instance
            .activate(
                |_, _| (),
                clack_host::prelude::PluginAudioConfiguration {
                    sample_rate: 48_000.0,
                    min_frames_count: 64,
                    max_frames_count: 64,
                },
            )
            .map_err(|error| HostError::Processing(error.to_string()))?;
        let mut processor = stopped
            .start_processing()
            .map_err(|error| HostError::Processing(error.to_string()))?;
        let mut inputs = [[0.25_f32; 64], [-0.25_f32; 64]];
        let mut outputs = [[0.0_f32; 64]; 2];
        let mut input_ports = clack_host::prelude::AudioPorts::with_capacity(2, 1);
        let mut output_ports = clack_host::prelude::AudioPorts::with_capacity(2, 1);
        let input_audio = input_ports.with_input_buffers([clack_host::prelude::AudioPortBuffer {
            latency: 0,
            channels: clack_host::prelude::AudioPortBufferType::f32_input_only(
                inputs
                    .iter_mut()
                    .map(clack_host::prelude::InputChannel::variable),
            ),
        }]);
        let mut output_audio =
            output_ports.with_output_buffers([clack_host::prelude::AudioPortBuffer {
                latency: 0,
                channels: clack_host::prelude::AudioPortBufferType::f32_output_only(
                    outputs.iter_mut().map(|channel| channel.as_mut_slice()),
                ),
            }]);
        let input_events = clack_host::prelude::InputEvents::empty();
        let mut output_buffer = clack_host::prelude::EventBuffer::new();
        let mut output_events = output_buffer.as_output();
        processor
            .process(
                &input_audio,
                &mut output_audio,
                &input_events,
                &mut output_events,
                None,
                None,
            )
            .map_err(|error| HostError::Processing(error.to_string()))?;
        let first_output = [outputs[0][0], outputs[1][0]];
        let stopped = processor.stop_processing();
        let mut saved = Vec::new();
        {
            let mut plugin = instance.plugin_handle();
            if let Some(state) = plugin.get_extension::<PluginState>() {
                state
                    .save(&mut plugin, &mut saved)
                    .map_err(|error| HostError::State(error.to_string()))?;
            }
        }
        instance.deactivate(stopped);
        Ok(ClapAudioSmokeResult {
            first_output,
            state_bytes: saved.len(),
        })
    }

    pub fn smoke_test_clap_gui(path: impl AsRef<Path>) -> Result<(u32, u32), HostError> {
        smoke_test_clap_gui_id(path, "ai.konko.ghost-agent-host")
    }

    pub fn smoke_test_clap_gui_id(
        path: impl AsRef<Path>,
        plugin_id: &str,
    ) -> Result<(u32, u32), HostError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = path;
            return Err(HostError::Scan(
                "The embedded GUI smoke test currently requires Windows".into(),
            ));
        }

        #[cfg(target_os = "windows")]
        {
            let entry = unsafe { PluginEntry::load(path.as_ref()) }
                .map_err(|error| HostError::Scan(error.to_string()))?;
            let host_info = HostInfo::new(
                "Ghost CLAP Validator",
                "Konko",
                "https://github.com/free-audio/clap",
                env!("CARGO_PKG_VERSION"),
            )
            .map_err(|error| HostError::Scan(error.to_string()))?;
            let plugin_id = std::ffi::CString::new(plugin_id)
                .map_err(|error| HostError::Scan(error.to_string()))?;
            let mut instance = PluginInstance::<ValidationHost>::new(
                |_| (),
                |_| (),
                &entry,
                &plugin_id,
                &host_info,
            )
            .map_err(|error| HostError::Scan(error.to_string()))?;
            let parent = ValidationWindow::create()?;
            let size = {
                let mut plugin = instance.plugin_handle();
                let gui = plugin.get_extension::<PluginGui>().ok_or_else(|| {
                    HostError::Scan("CLAP plugin does not expose clap.gui".into())
                })?;
                let configuration = GuiConfiguration {
                    api_type: GuiApiType::WIN32,
                    is_floating: false,
                };
                if !gui.is_api_supported(&mut plugin, configuration) {
                    return Err(HostError::Scan(
                        "CLAP plugin rejected an embedded Win32 editor".into(),
                    ));
                }

                let mut reported_size = None;
                for _ in 0..2 {
                    gui.create(&mut plugin, configuration)
                        .map_err(|error| HostError::Scan(error.to_string()))?;
                    let size = gui.get_size(&mut plugin).ok_or_else(|| {
                        HostError::Scan("CLAP GUI returned an invalid initial size".into())
                    })?;
                    // SAFETY: ValidationWindow remains alive until after gui.destroy below.
                    unsafe { gui.set_parent(&mut plugin, Window::from_win32_hwnd(parent.0)) }
                        .map_err(|error| HostError::Scan(error.to_string()))?;
                    let child = parent.plugin_child().ok_or_else(|| {
                        HostError::Scan("CLAP GUI did not create a Win32 child window".into())
                    })?;
                    for _ in 0..2 {
                        gui.show(&mut plugin)
                            .map_err(|error| HostError::Scan(error.to_string()))?;
                        pump_messages_for(std::time::Duration::from_millis(75));
                        if !child_is_shown(child) {
                            return Err(HostError::Scan(
                                "CLAP GUI child remained hidden after show".into(),
                            ));
                        }
                        gui.hide(&mut plugin)
                            .map_err(|error| HostError::Scan(error.to_string()))?;
                        pump_messages_for(std::time::Duration::from_millis(25));
                        if child_is_shown(child) {
                            return Err(HostError::Scan(
                                "CLAP GUI child remained visible after hide".into(),
                            ));
                        }
                    }
                    gui.destroy(&mut plugin);
                    pump_messages_for(std::time::Duration::from_millis(25));
                    if parent.plugin_child().is_some() {
                        return Err(HostError::Scan(
                            "CLAP GUI child remained alive after destroy".into(),
                        ));
                    }
                    reported_size = Some(size);
                }
                reported_size.expect("the GUI validation loop always runs")
            };
            Ok((size.width, size.height))
        }
    }
}

pub fn is_expected_fabfilter(record: &PluginDescriptorRecord) -> bool {
    let normalized = format!("{} {}", record.id, record.name).to_lowercase();
    normalized.contains("fabfilter")
        && (normalized.contains("pro-q 4")
            || normalized.contains("pro q 4")
            || normalized.contains("pro-c 3")
            || normalized.contains("pro c 3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_state_round_trip() {
        let chain = MockFabFilterChain::default();
        let state = chain.save_state().unwrap();
        let mut restored = MockFabFilterChain::default();
        restored.load_state(&state).unwrap();
        assert_eq!(state.schema_version, "ghost.chain-state/1");
    }
}
