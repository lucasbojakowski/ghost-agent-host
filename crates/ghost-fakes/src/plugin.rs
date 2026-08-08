//! Loadable CLAP child with deterministic audio ports, gain parameter, and state.

use std::ffi::CStr;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use clack_common::events::event_types::ParamValueEvent;
use clack_common::events::io::{InputEvents, OutputEvents};
use clack_common::events::Pckn;
use clack_common::utils::{ClapId, Cookie};
use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
    PluginAudioPortsImpl,
};
#[cfg(target_os = "windows")]
use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiSize, PluginGui, PluginGuiImpl, Window,
};
use clack_extensions::params::{
    ParamDisplayWriter, ParamInfo, ParamInfoFlags, ParamInfoWriter, PluginAudioProcessorParams,
    PluginMainThreadParams, PluginParams,
};
use clack_extensions::state::{PluginState, PluginStateImpl};
use clack_plugin::plugin::features::{AUDIO_EFFECT, STEREO, UTILITY};
use clack_plugin::prelude::*;

const GAIN_ID: ClapId = ClapId::new(1);

pub struct FakeChildPlugin;

pub struct FakeShared {
    gain: AtomicU64,
}

impl Default for FakeShared {
    fn default() -> Self {
        Self {
            gain: AtomicU64::new(1.0_f64.to_bits()),
        }
    }
}

impl PluginShared<'_> for FakeShared {}

pub struct FakeMainThread<'a> {
    shared: &'a FakeShared,
    #[cfg(target_os = "windows")]
    gui: FakeGui,
}

impl<'a> PluginMainThread<'a, FakeShared> for FakeMainThread<'a> {}

impl Plugin for FakeChildPlugin {
    type AudioProcessor<'a> = FakeAudioProcessor<'a>;
    type Shared<'a> = FakeShared;
    type MainThread<'a> = FakeMainThread<'a>;

    fn declare_extensions(
        builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        builder.register::<PluginAudioPorts>();
        builder.register::<PluginParams>();
        builder.register::<PluginState>();
        #[cfg(target_os = "windows")]
        builder.register::<PluginGui>();
    }
}

impl DefaultPluginFactory for FakeChildPlugin {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new(super::child::FAKE_CHILD_ID, "Ghost Fake CLAP Child")
            .with_vendor("Konko")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_description("Deterministic gain child for native host integration tests")
            .with_features([AUDIO_EFFECT, UTILITY, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(FakeShared::default())
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(FakeMainThread {
            shared,
            #[cfg(target_os = "windows")]
            gui: FakeGui::default(),
        })
    }
}

impl PluginAudioPortsImpl for FakeMainThread<'_> {
    fn count(&mut self, _is_input: bool) -> u32 {
        1
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
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
                in_place_pair: Some(ClapId::new(0)),
            });
        }
    }
}

impl PluginMainThreadParams for FakeMainThread<'_> {
    fn count(&mut self) -> u32 {
        1
    }

    fn get_info(&mut self, param_index: u32, writer: &mut ParamInfoWriter) {
        if param_index == 0 {
            writer.set(&ParamInfo {
                id: GAIN_ID,
                flags: ParamInfoFlags::IS_AUTOMATABLE,
                cookie: Cookie::empty(),
                name: b"Output gain",
                module: b"Output",
                min_value: 0.0,
                max_value: 2.0,
                default_value: 1.0,
            });
        }
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        (param_id == GAIN_ID).then(|| f64::from_bits(self.shared.gain.load(Ordering::Acquire)))
    }

    fn value_to_text(
        &mut self,
        param_id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        if param_id != GAIN_ID {
            return Err(std::fmt::Error);
        }
        write!(writer, "{value:.2}×")
    }

    fn text_to_value(&mut self, param_id: ClapId, text: &CStr) -> Option<f64> {
        (param_id == GAIN_ID)
            .then(|| text.to_string_lossy().trim_end_matches('×').parse().ok())
            .flatten()
    }

    fn flush(&mut self, input: &InputEvents, output: &mut OutputEvents) {
        flush_gain(self.shared, input, output);
    }
}

impl PluginStateImpl for FakeMainThread<'_> {
    fn save(&mut self, output: &mut clack_common::stream::OutputStream) -> Result<(), PluginError> {
        output.write_all(&self.shared.gain.load(Ordering::Acquire).to_le_bytes())?;
        Ok(())
    }

    fn load(&mut self, input: &mut clack_common::stream::InputStream) -> Result<(), PluginError> {
        let mut bytes = [0; 8];
        input.read_exact(&mut bytes)?;
        self.shared
            .gain
            .store(u64::from_le_bytes(bytes), Ordering::Release);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[derive(Default)]
struct FakeGui {
    created: bool,
    hwnd: windows_sys::Win32::Foundation::HWND,
}

#[cfg(target_os = "windows")]
impl PluginGuiImpl for FakeMainThread<'_> {
    fn is_api_supported(&mut self, configuration: GuiConfiguration) -> bool {
        configuration.api_type == GuiApiType::WIN32 && !configuration.is_floating
    }

    fn get_preferred_api(&mut self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::WIN32,
            is_floating: false,
        })
    }

    fn create(&mut self, configuration: GuiConfiguration) -> Result<(), PluginError> {
        if !self.is_api_supported(configuration) || self.gui.created {
            return Err(PluginError::Message("Unsupported or duplicate fake GUI"));
        }
        self.gui.created = true;
        Ok(())
    }

    fn destroy(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyWindow, IsWindow};
        // SAFETY: hwnd, when non-null, was created by this FakeGui on the CLAP main thread.
        unsafe {
            if !self.gui.hwnd.is_null() && IsWindow(self.gui.hwnd) != 0 {
                DestroyWindow(self.gui.hwnd);
            }
        }
        self.gui.hwnd = std::ptr::null_mut();
        self.gui.created = false;
    }

    fn set_scale(&mut self, _scale: f64) -> Result<(), PluginError> {
        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        Some(GuiSize {
            width: 420,
            height: 120,
        })
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
        };
        if self.gui.hwnd.is_null() {
            return Ok(());
        }
        // SAFETY: hwnd is the live child window owned by this fake GUI.
        let result = unsafe {
            SetWindowPos(
                self.gui.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                size.width as i32,
                size.height as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        (result != 0)
            .then_some(())
            .ok_or(PluginError::Message("Failed to resize fake GUI"))
    }

    fn set_parent(&mut self, parent: Window) -> Result<(), PluginError> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WS_CHILD};
        if !self.gui.created || !self.gui.hwnd.is_null() {
            return Err(PluginError::Message("Fake GUI is not ready for a parent"));
        }
        let parent = parent
            .as_win32_hwnd()
            .ok_or(PluginError::Message("Expected a Win32 parent"))?
            as windows_sys::Win32::Foundation::HWND;
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let text: Vec<u16> = "Ghost Fake Child · native GUI lifecycle is active\0"
            .encode_utf16()
            .collect();
        // SAFETY: class/text are terminated UTF-16 and parent is supplied by the CLAP host.
        self.gui.hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                text.as_ptr(),
                WS_CHILD,
                0,
                0,
                420,
                120,
                parent,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        (!self.gui.hwnd.is_null())
            .then_some(())
            .ok_or(PluginError::Message("Failed to create fake child window"))
    }

    fn set_transient(&mut self, _window: Window) -> Result<(), PluginError> {
        Err(PluginError::Message("Fake GUI is embedded only"))
    }

    fn show(&mut self) -> Result<(), PluginError> {
        self.show_window(true)
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        self.show_window(false)
    }
}

#[cfg(target_os = "windows")]
impl FakeMainThread<'_> {
    fn show_window(&mut self, visible: bool) -> Result<(), PluginError> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};
        if self.gui.hwnd.is_null() {
            return Err(PluginError::Message("Fake GUI has no parent"));
        }
        // SAFETY: hwnd is the live child window owned by this fake GUI.
        unsafe {
            ShowWindow(self.gui.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for FakeMainThread<'_> {
    fn drop(&mut self) {
        self.destroy();
    }
}

pub struct FakeAudioProcessor<'a> {
    shared: &'a FakeShared,
}

impl<'a> PluginAudioProcessor<'a, FakeShared, FakeMainThread<'a>> for FakeAudioProcessor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut FakeMainThread<'a>,
        shared: &'a FakeShared,
        _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self { shared })
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        let mut feedback = None;
        for event in events.input {
            if let Some(value) = event.as_event::<ParamValueEvent>() {
                if value.param_id() == Some(GAIN_ID) {
                    let value = value.value().clamp(0.0, 2.0);
                    self.shared.gain.store(value.to_bits(), Ordering::Release);
                    feedback = Some(value);
                }
            }
        }
        if let Some(value) = feedback {
            let _ = events.output.try_push(ParamValueEvent::new(
                0,
                GAIN_ID,
                Pckn::match_all(),
                value,
                Cookie::empty(),
            ));
        }
        let gain = f64::from_bits(self.shared.gain.load(Ordering::Acquire)) as f32;
        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };
            for channel_pair in channel_pairs {
                match channel_pair {
                    ChannelPair::InputOnly(_) => {}
                    ChannelPair::OutputOnly(output) => output.fill(0.0),
                    ChannelPair::InputOutput(input, output) => {
                        for (input, output) in input.iter().zip(output) {
                            *output = *input * gain;
                        }
                    }
                    ChannelPair::InPlace(buffer) => {
                        for sample in buffer {
                            *sample *= gain;
                        }
                    }
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

impl PluginAudioProcessorParams for FakeAudioProcessor<'_> {
    fn flush(&mut self, input: &InputEvents, output: &mut OutputEvents) {
        flush_gain(self.shared, input, output);
    }
}

fn flush_gain(shared: &FakeShared, input: &InputEvents, output: &mut OutputEvents) {
    for event in input {
        let Some(value) = event.as_event::<ParamValueEvent>() else {
            continue;
        };
        if value.param_id() != Some(GAIN_ID) {
            continue;
        }
        let value = value.value().clamp(0.0, 2.0);
        shared.gain.store(value.to_bits(), Ordering::Release);
        let _ = output.try_push(ParamValueEvent::new(
            0,
            GAIN_ID,
            Pckn::match_all(),
            value,
            Cookie::empty(),
        ));
    }
}

clack_export_entry!(SinglePluginEntry<FakeChildPlugin>);
