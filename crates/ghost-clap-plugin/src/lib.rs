//! Minimal DAW-loadable outer CLAP shell.
//!
//! The shell is deliberately realtime-safe and transparent. The full nested
//! host is connected through the `ghost-host` adapter in the integration
//! branch because proprietary child binaries and DAW callbacks are required
//! to validate lifecycle, GUI parenting, latency, and state behavior.

use clack_plugin::plugin::features::{ANALYZER, AUDIO_EFFECT, STEREO};
use clack_plugin::prelude::*;

pub struct GhostAgentHostPlugin;

impl Plugin for GhostAgentHostPlugin {
    type AudioProcessor<'a> = GhostAudioProcessor;
    type Shared<'a> = ();
    type MainThread<'a> = ();
}

impl DefaultPluginFactory for GhostAgentHostPlugin {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new("ai.konko.ghost-agent-host", "Ghost Agent Host")
            .with_vendor("Konko")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_description("AI-assisted audio analysis and child-plugin host")
            .with_features([AUDIO_EFFECT, ANALYZER, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(())
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(())
    }
}

pub struct GhostAudioProcessor;

impl<'a> PluginAudioProcessor<'a, (), ()> for GhostAudioProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut (),
        _shared: &'a (),
        _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };
            for channel_pair in channel_pairs {
                match channel_pair {
                    ChannelPair::InputOnly(_) => {}
                    ChannelPair::OutputOnly(output) => output.fill(0.0),
                    ChannelPair::InputOutput(input, output) => {
                        output.copy_from_slice(input);
                    }
                    ChannelPair::InPlace(_) => {}
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

clack_export_entry!(SinglePluginEntry<GhostAgentHostPlugin>);
