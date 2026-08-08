use std::collections::BTreeMap;
use std::ffi::CStr;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use clack_common::stream::{InputStream, OutputStream};
use clack_extensions::audio_ports::{AudioPortInfoWriter, PluginAudioPortsImpl};
use clack_extensions::gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGuiImpl, Window};
use clack_extensions::latency::PluginLatencyImpl;
use clack_extensions::params::{ParamDisplayWriter, ParamInfoWriter, PluginMainThreadParams};
use clack_extensions::state::{HostState, PluginStateImpl};
use clack_plugin::prelude::{HostMainThreadHandle, PluginError, PluginMainThread};
use egui::{FullOutput, ViewportOutput};
use egui_baseview::baseview::dpi::{PhysicalSize, Size};
use egui_baseview::baseview::{WindowHandle, WindowScalePolicy};
use egui_baseview::{EguiWindow, EguiWindowSettings, ExtraOutputCommands};
use ghost_core::{capture_post_tap_key, AtomicDawState, AtomicGraphControl, RealtimeCaptureBuffer};
use ghost_host::{NativeClapAudio, NativeClapMain, ProcessConfig};
use ghost_ui::{
    GhostUi, HostControl, PersistedUiState, UiSession, DEFAULT_EDITOR_HEIGHT, DEFAULT_EDITOR_WIDTH,
};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, FindWindowExW, GetParent, IsWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
};

const EDITOR_TITLE: &str = "Ghost Agent Host";

pub struct GhostEditorMainThread {
    created: bool,
    window: Option<EditorWindow>,
    size: GuiSize,
    scale: f64,
    persisted: Arc<Mutex<PersistedUiState>>,
    session: Arc<Mutex<UiSession>>,
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    graph_control: Arc<AtomicGraphControl>,
    native_children: Vec<(String, NativeClapMain)>,
    child_windows: BTreeMap<String, crate::child_window::DetachedChildWindow>,
    host_parent: Option<HWND>,
    commands: Arc<Mutex<Vec<crate::MainThreadCommand>>>,
    host_control: Arc<dyn HostControl>,
    outer_host: HostMainThreadHandle<'static>,
    nested_bridge: Arc<dyn ghost_host::NestedHostBridge>,
    latency_samples: u32,
    parameter_control: Arc<ghost_host::RealtimeParameterControl>,
}

impl GhostEditorMainThread {
    pub fn new(shared: &crate::GhostShared, outer_host: HostMainThreadHandle<'_>) -> Self {
        let host_control: Arc<dyn HostControl> = Arc::new(crate::ClapHostControl {
            host: shared.host,
            commands: Arc::clone(&shared.commands),
            parameter_control: Arc::clone(&shared.parameter_control),
        });
        let nested_bridge: Arc<dyn ghost_host::NestedHostBridge> =
            Arc::new(crate::OuterNestedHostBridge { host: shared.host });
        // SAFETY: this handle is stored only in the main-thread object and dies with the plugin.
        let outer_host = unsafe { outer_host.with_arbitrary_lifetime() };
        Self {
            created: false,
            window: None,
            size: GuiSize {
                width: DEFAULT_EDITOR_WIDTH,
                height: DEFAULT_EDITOR_HEIGHT,
            },
            scale: 1.0,
            persisted: Arc::new(Mutex::new(PersistedUiState::default())),
            session: Arc::new(Mutex::new(UiSession::default())),
            daw: Arc::clone(&shared.daw),
            capture: Arc::clone(&shared.capture),
            graph_control: Arc::clone(&shared.graph_control),
            native_children: Vec::new(),
            child_windows: BTreeMap::new(),
            host_parent: None,
            commands: Arc::clone(&shared.commands),
            host_control,
            outer_host,
            nested_bridge,
            latency_samples: 0,
            parameter_control: Arc::clone(&shared.parameter_control),
        }
    }
}

impl PluginMainThread<'_, crate::GhostShared> for GhostEditorMainThread {
    fn on_main_thread(&mut self) {
        self.service_nested_host_events();
        let commands = self
            .commands
            .lock()
            .map(|mut commands| std::mem::take(&mut *commands))
            .unwrap_or_default();
        for command in commands {
            let (node_id, visible) = match command {
                crate::MainThreadCommand::ShowChildGui(id) => (id, true),
                crate::MainThreadCommand::HideChildGui(id) => (id, false),
                crate::MainThreadCommand::SyncChildStates => {
                    self.sync_child_states();
                    continue;
                }
                crate::MainThreadCommand::MarkDirty => {
                    if let Some(mut state) = self.outer_host.get_extension::<HostState>() {
                        state.mark_dirty(&self.outer_host);
                    }
                    continue;
                }
            };
            let result = if visible {
                self.open_child_gui(&node_id)
            } else {
                self.hide_child_gui(&node_id)
            };
            if let Ok(mut session) = self.session.lock() {
                session.set_runtime_notice(match result {
                    Ok(()) => format!(
                        "Child UI {} for {node_id}",
                        if visible { "opened" } else { "hidden" }
                    ),
                    Err(error) => format!("Child UI: {error}"),
                });
            }
        }
    }
}

impl PluginMainThreadParams for GhostEditorMainThread {
    fn count(&mut self) -> u32 {
        0
    }

    fn get_info(&mut self, _param_index: u32, _info: &mut ParamInfoWriter) {}

    fn get_value(&mut self, _param_id: clack_common::utils::ClapId) -> Option<f64> {
        None
    }

    fn value_to_text(
        &mut self,
        _param_id: clack_common::utils::ClapId,
        _value: f64,
        _writer: &mut ParamDisplayWriter,
    ) -> core::fmt::Result {
        Err(core::fmt::Error)
    }

    fn text_to_value(
        &mut self,
        _param_id: clack_common::utils::ClapId,
        _text: &CStr,
    ) -> Option<f64> {
        None
    }

    fn flush(
        &mut self,
        _input_parameter_changes: &clack_common::events::io::InputEvents,
        _output_parameter_changes: &mut clack_common::events::io::OutputEvents,
    ) {
        self.apply_inactive_parameter_commands();
    }
}

impl GhostEditorMainThread {
    fn apply_inactive_parameter_commands(&mut self) {
        let active_revision = self
            .persisted
            .lock()
            .map(|document| document.graph_revision)
            .unwrap_or_default();
        while let Some(transaction) = self.parameter_control.pop_transaction() {
            let rejection = if transaction.expected_graph_revision != active_revision {
                Some((0, ghost_host::ParameterAckStatus::GraphRevisionMismatch))
            } else {
                transaction
                    .changes
                    .iter()
                    .enumerate()
                    .find_map(|(index, change)| {
                        match self
                            .native_children
                            .iter()
                            .find(|(node_id, _)| node_id == &change.target_node_id)
                        {
                            None => Some((index, ghost_host::ParameterAckStatus::NodeUnavailable)),
                            Some((_, child)) => child
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
                let (status, previous_value) = self
                    .native_children
                    .iter_mut()
                    .find(|(node_id, _)| node_id == &change.target_node_id)
                    .map_or(
                        (ghost_host::ParameterAckStatus::NodeUnavailable, None),
                        |(_, child)| {
                            child
                                .set_parameter_plain_inactive(
                                    &change.parameter_id,
                                    change.plain_value,
                                )
                                .map_or_else(
                                    |_| (ghost_host::ParameterAckStatus::ParameterRejected, None),
                                    |previous| {
                                        (ghost_host::ParameterAckStatus::Applied, Some(previous))
                                    },
                                )
                        },
                    );
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

    pub(crate) fn activate_children(
        &mut self,
        config: ProcessConfig,
    ) -> (u64, Vec<(String, u64, u64, NativeClapAudio)>) {
        self.native_children.clear();
        self.child_windows.clear();
        let (graph, graph_revision) = self
            .persisted
            .lock()
            .map(|state| (state.graph.clone(), state.graph_revision))
            .unwrap_or_default();
        let bypass_mask = graph
            .nodes
            .iter()
            .take(64)
            .enumerate()
            .fold(0_u64, |mask, (index, node)| {
                mask | ((node.bypassed as u64) << index)
            });
        self.graph_control.set_bypass_mask(bypass_mask);
        let mut audio_children = Vec::new();
        let mut latency_samples = 0_u32;
        let mut failures = Vec::new();
        for (node_index, node) in graph.nodes.into_iter().enumerate() {
            let Some(plugin) = node.plugin else {
                continue;
            };
            let Some(plugin_id) = plugin.plugin_id else {
                failures.push(format!("{} has no CLAP ID", plugin.name));
                continue;
            };
            match NativeClapMain::open_with_bridge(
                &plugin.path,
                &plugin_id,
                Arc::clone(&self.nested_bridge),
            )
            .and_then(|mut main| {
                if let Some(state) = &plugin.state {
                    main.load_state(state)?;
                }
                let values = main.parameter_values();
                main.activate(config).map(|audio| (main, audio, values))
            }) {
                Ok((main, audio, values)) => {
                    let mut main = main;
                    latency_samples = latency_samples.saturating_add(main.latency_samples());
                    self.native_children.push((node.id.clone(), main));
                    if let Ok(mut session) = self.session.lock() {
                        for (parameter_id, value) in values {
                            session.record_parameter_feedback(node.id.clone(), parameter_id, value);
                        }
                    }
                    let post_key = capture_post_tap_key(&node.id);
                    let bypass_bit = if node_index < 64 {
                        1_u64 << node_index
                    } else {
                        0
                    };
                    audio_children.push((node.id, post_key, bypass_bit, audio));
                }
                Err(error) => failures.push(format!("{}: {error}", plugin.name)),
            }
        }
        if let Ok(mut session) = self.session.lock() {
            session.set_runtime_notice(if failures.is_empty() {
                format!("{} native child nodes active", audio_children.len())
            } else {
                format!(
                    "{} native children active · {} failed: {}",
                    audio_children.len(),
                    failures.len(),
                    failures.join("; ")
                )
            });
            session.graph_activated(graph_revision);
        }
        self.latency_samples = latency_samples;
        (graph_revision, audio_children)
    }

    pub(crate) fn deactivate_children(
        &mut self,
        children: Vec<(String, u64, u64, NativeClapAudio)>,
    ) {
        self.sync_child_states();
        for (id, _, _, audio) in children {
            if let Some(index) = self
                .native_children
                .iter()
                .position(|(child_id, _)| child_id == &id)
            {
                let (_, mut main) = self.native_children.remove(index);
                main.deactivate(audio);
                self.child_windows.remove(&id);
            }
        }
        self.native_children.clear();
        self.child_windows.clear();
    }

    fn sync_child_states(&mut self) {
        let saved: Vec<_> = self
            .native_children
            .iter_mut()
            .filter_map(|(id, main)| main.save_state().ok().map(|state| (id.clone(), state)))
            .collect();
        if let Ok(mut persisted) = self.persisted.lock() {
            for (id, child_state) in saved {
                if let Some(plugin) = persisted
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == id)
                    .and_then(|node| node.plugin.as_mut())
                {
                    plugin.state = Some(child_state);
                }
            }
        }
    }

    fn service_nested_host_events(&mut self) {
        let mut events = Vec::new();
        for (node_id, child) in &mut self.native_children {
            events.extend(
                child
                    .service_host_callbacks()
                    .into_iter()
                    .map(|event| (node_id.clone(), event)),
            );
        }
        for (node_id, event) in events {
            match event {
                ghost_host::NestedHostEvent::GuiShowRequested => {
                    if let Some((_, child)) = self
                        .native_children
                        .iter_mut()
                        .find(|(id, _)| id == &node_id)
                    {
                        let _ = child.set_gui_visible(true);
                    }
                }
                ghost_host::NestedHostEvent::GuiHideRequested => {
                    if let Some((_, child)) = self
                        .native_children
                        .iter_mut()
                        .find(|(id, _)| id == &node_id)
                    {
                        let _ = child.set_gui_visible(false);
                    }
                }
                ghost_host::NestedHostEvent::GuiResizeRequested(size) => {
                    if let Some(window) = self.child_windows.get(&node_id) {
                        let _ = window.resize_client(size);
                    }
                    if let Some((_, child)) = self
                        .native_children
                        .iter_mut()
                        .find(|(id, _)| id == &node_id)
                    {
                        let _ = child.set_gui_size(size);
                    }
                }
                ghost_host::NestedHostEvent::GuiClosed { was_destroyed } => {
                    if let Some((_, child)) = self
                        .native_children
                        .iter_mut()
                        .find(|(id, _)| id == &node_id)
                    {
                        child.acknowledge_gui_closed(was_destroyed);
                    }
                    if was_destroyed {
                        self.child_windows.remove(&node_id);
                    } else if let Some(window) = self.child_windows.get(&node_id) {
                        window.show(false);
                    }
                }
                ghost_host::NestedHostEvent::ParametersRescan { .. } => {
                    if let Some((_, child)) = self
                        .native_children
                        .iter_mut()
                        .find(|(id, _)| id == &node_id)
                    {
                        if child.refresh_parameter_manifest().is_ok() {
                            let parameters = child.descriptor().parameters.clone();
                            if let Ok(mut document) = self.persisted.lock() {
                                if let Some(plugin) = document
                                    .graph
                                    .nodes
                                    .iter_mut()
                                    .find(|node| node.id == node_id)
                                    .and_then(|node| node.plugin.as_mut())
                                {
                                    plugin.public_parameters = parameters;
                                }
                            }
                        }
                    }
                }
                ghost_host::NestedHostEvent::StateDirty => {
                    if let Some(mut state) = self.outer_host.get_extension::<HostState>() {
                        state.mark_dirty(&self.outer_host);
                    }
                }
                ghost_host::NestedHostEvent::LatencyChanged => {
                    self.host_control.request_graph_restart();
                    if let Ok(mut session) = self.session.lock() {
                        session.set_runtime_notice(format!(
                            "{node_id} changed latency; graph restart requested"
                        ));
                    }
                }
                ghost_host::NestedHostEvent::ParameterValue {
                    parameter_id,
                    value,
                } => {
                    if let Ok(mut session) = self.session.lock() {
                        session.record_parameter_feedback(
                            node_id.clone(),
                            parameter_id.to_string(),
                            value,
                        );
                    }
                }
                ghost_host::NestedHostEvent::Log { severity, message } => {
                    if let Ok(mut session) = self.session.lock() {
                        session
                            .set_runtime_notice(format!("Child {node_id} [{severity}]: {message}"));
                    }
                }
                ghost_host::NestedHostEvent::GuiResizeHintsChanged
                | ghost_host::NestedHostEvent::ParameterClear { .. } => {}
            }
        }
    }

    fn open_child_gui(&mut self, node_id: &str) -> Result<(), String> {
        let index = self
            .native_children
            .iter()
            .position(|(id, _)| id == node_id)
            .ok_or_else(|| "Child is not active; wait for the DAW graph restart".to_owned())?;
        let title = format!(
            "Ghost · {}",
            self.native_children[index].1.descriptor().name
        );
        match self.native_children[index].1.preferred_gui_mode() {
            Some(ghost_host::ChildGuiMode::PluginFloating) => {
                let owner = self
                    .host_parent
                    .ok_or_else(|| "The DAW owner window is not available".to_owned())?;
                self.native_children[index]
                    .1
                    .open_floating_gui(ghost_host::ParentWindow { raw: owner.cast() }, &title)
                    .map_err(|error| error.to_string())
            }
            Some(ghost_host::ChildGuiMode::HostedDetached) => {
                let size = self.native_children[index].1.gui_size().unwrap_or(
                    clack_extensions::gui::GuiSize {
                        width: 640,
                        height: 420,
                    },
                );
                if !self.child_windows.contains_key(node_id) {
                    let window = crate::child_window::DetachedChildWindow::create(&title, size)
                        .map_err(|error| error.to_string())?;
                    self.native_children[index]
                        .1
                        .open_gui(ghost_host::ParentWindow {
                            raw: window.hwnd().cast(),
                        })
                        .map_err(|error| error.to_string())?;
                    self.child_windows.insert(node_id.to_owned(), window);
                } else {
                    self.native_children[index]
                        .1
                        .set_gui_visible(true)
                        .map_err(|error| error.to_string())?;
                }
                if let Some(window) = self.child_windows.get(node_id) {
                    window.show(true);
                }
                Ok(())
            }
            None => Err("Child exposes neither floating nor embedded Win32 GUI mode".into()),
        }
    }

    fn hide_child_gui(&mut self, node_id: &str) -> Result<(), String> {
        let (_, child) = self
            .native_children
            .iter_mut()
            .find(|(id, _)| id == node_id)
            .ok_or_else(|| "Child is not active; wait for the DAW graph restart".to_owned())?;
        child
            .set_gui_visible(false)
            .map_err(|error| error.to_string())?;
        if let Some(window) = self.child_windows.get(node_id) {
            window.show(false);
        }
        Ok(())
    }
}

impl PluginAudioPortsImpl for GhostEditorMainThread {
    fn count(&mut self, _is_input: bool) -> u32 {
        1
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        crate::write_stereo_port(index, is_input, writer);
    }
}

impl PluginGuiImpl for GhostEditorMainThread {
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
        if !self.is_api_supported(configuration) {
            return Err(PluginError::Message(
                "Ghost Agent Host only supports embedded Win32 editors",
            ));
        }
        if self.created {
            return Err(PluginError::Message("Ghost editor already exists"));
        }

        self.created = true;
        Ok(())
    }

    fn destroy(&mut self) {
        self.window.take();
        self.created = false;
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PluginError::Message("Invalid editor scale"));
        }
        self.scale = scale;
        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        Some(self.size)
    }

    fn can_resize(&mut self) -> bool {
        true
    }

    fn adjust_size(&mut self, size: GuiSize) -> Option<GuiSize> {
        (size.width >= 860 && size.height >= 600).then_some(size)
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        let size = self
            .adjust_size(size)
            .ok_or(PluginError::Message("Editor size could not be adjusted"))?;
        self.size = size;
        if let Ok(mut state) = self.persisted.lock() {
            state.editor_width = size.width;
            state.editor_height = size.height;
        }
        if let Some(window) = &self.window {
            window.resize(size)?;
        }
        Ok(())
    }

    fn set_parent(&mut self, parent: Window) -> Result<(), PluginError> {
        if !self.created {
            return Err(PluginError::Message("Editor has not been created"));
        }
        if self.window.is_some() {
            return Err(PluginError::Message("Ghost editor already has a parent"));
        }

        let parent_hwnd = parent
            .as_win32_hwnd()
            .ok_or(PluginError::Message("Host did not provide a Win32 parent"))?
            as HWND;
        self.host_parent = Some(parent_hwnd);
        let settings = EguiWindowSettings::new()
            .with_tile(EDITOR_TITLE)
            .with_size(Size::Physical(PhysicalSize {
                width: self.size.width,
                height: self.size.height,
            }))
            .with_scale_policy(WindowScalePolicy::ScaleFactor(self.scale));
        // SAFETY: The host guarantees that its parent window remains valid through gui.destroy.
        let parent_handle = unsafe { parent.borrow_handle_unchecked() }
            .map_err(|error| io::Error::other(error.to_string()))?;
        let handle = EguiWindow::open_parented(
            &parent_handle,
            settings,
            GhostUi::with_runtime_and_session(
                Arc::clone(&self.persisted),
                Arc::clone(&self.session),
                Arc::clone(&self.daw),
                Arc::clone(&self.capture),
                Arc::clone(&self.graph_control),
                Arc::clone(&self.host_control),
            ),
            |_context, _commands: &mut ExtraOutputCommands, _state| {},
            |_output: &FullOutput, _viewport: &ViewportOutput, _state| {},
            |ui, _commands: &mut ExtraOutputCommands, state| state.show(ui),
        );

        let hwnd = find_editor_child(parent_hwnd).ok_or_else(|| {
            handle.close();
            io::Error::other("egui-baseview did not create the editor child window")
        })?;
        let editor = EditorWindow {
            hwnd,
            _handle: handle,
        };
        editor.show(false);
        self.window = Some(editor);
        Ok(())
    }

    fn set_transient(&mut self, _window: Window) -> Result<(), PluginError> {
        Err(PluginError::Message("Floating editors are not supported"))
    }

    fn show(&mut self) -> Result<(), PluginError> {
        self.window
            .as_ref()
            .ok_or(PluginError::Message("Editor does not have a parent"))?
            .show(true);
        Ok(())
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        self.window
            .as_ref()
            .ok_or(PluginError::Message("Editor does not have a parent"))?
            .show(false);
        Ok(())
    }
}

impl PluginLatencyImpl for GhostEditorMainThread {
    fn get(&mut self) -> u32 {
        self.latency_samples
    }
}

impl Drop for GhostEditorMainThread {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl PluginStateImpl for GhostEditorMainThread {
    fn save(&mut self, output: &mut OutputStream) -> Result<(), PluginError> {
        self.sync_child_states();
        let state = self
            .persisted
            .lock()
            .map_err(|_| PluginError::Message("Ghost UI state lock was poisoned"))?;
        let encoded = serde_json::to_vec(&*state)
            .map_err(|_| PluginError::Message("Ghost UI state serialization failed"))?;
        output.write_all(&encoded)?;
        Ok(())
    }

    fn load(&mut self, input: &mut InputStream) -> Result<(), PluginError> {
        const MAXIMUM_STATE_BYTES: u64 = 16 * 1024 * 1024;
        let mut encoded = Vec::new();
        input.take(MAXIMUM_STATE_BYTES).read_to_end(&mut encoded)?;
        let state: PersistedUiState = serde_json::from_slice(&encoded)
            .map_err(|_| PluginError::Message("Ghost UI state blob is invalid"))?;
        if !matches!(
            state.schema_version.as_str(),
            "ghost.ui-state/1" | "ghost.ui-state/2" | "ghost.ui-state/3"
        ) {
            return Err(PluginError::Message(
                "Ghost UI state version is unsupported",
            ));
        }
        let revision = {
            let mut document = self
                .persisted
                .lock()
                .map_err(|_| PluginError::Message("Ghost UI state lock was poisoned"))?;
            let current_revision = document.graph_revision;
            let mut loaded = state.migrate();
            loaded.graph_revision = loaded
                .graph_revision
                .max(current_revision)
                .saturating_add(1);
            self.size = GuiSize {
                width: loaded.editor_width,
                height: loaded.editor_height,
            };
            let revision = loaded.graph_revision;
            *document = loaded;
            revision
        };
        if let Ok(mut session) = self.session.lock() {
            session.graph_committed(revision);
            session.set_runtime_notice(format!("Project loaded · graph r{revision} pending"));
        }
        self.host_control.request_graph_restart();
        Ok(())
    }
}

struct EditorWindow {
    hwnd: HWND,
    _handle: WindowHandle,
}

impl EditorWindow {
    fn resize(&self, size: GuiSize) -> Result<(), PluginError> {
        let width = i32::try_from(size.width)
            .map_err(|_| PluginError::Message("Editor width is too large"))?;
        let height = i32::try_from(size.height)
            .map_err(|_| PluginError::Message("Editor height is too large"))?;
        // SAFETY: hwnd is the live child window owned by the baseview handle.
        let succeeded = unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if succeeded == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    fn show(&self, visible: bool) {
        // SAFETY: hwnd is the live child window owned by the baseview handle.
        unsafe {
            ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        // baseview's public close method posts a message. CLAP may unload the DLL immediately after
        // destroy returns, so destroy the same-thread child synchronously instead.
        // SAFETY: CLAP invokes GUI lifecycle methods on the same main thread that created hwnd.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

fn find_editor_child(parent: HWND) -> Option<HWND> {
    let title: Vec<u16> = EDITOR_TITLE.encode_utf16().chain(Some(0)).collect();
    // SAFETY: parent is supplied by the host and title is a terminated UTF-16 string.
    let child = unsafe {
        FindWindowExW(
            parent,
            std::ptr::null_mut(),
            std::ptr::null(),
            title.as_ptr(),
        )
    };
    if child.is_null() || unsafe { GetParent(child) } != parent {
        None
    } else {
        Some(child)
    }
}
