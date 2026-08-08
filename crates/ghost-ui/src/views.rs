use ghost_core::RealtimeCaptureState;
use ghost_host::{GraphPreset, ProcessorClass};

use crate::state::{CaptureSource, PersistedUiState};
use crate::widgets::{self, accent, blue, orange, panel_frame, section};
use crate::{GhostUi, Stage, UiSession};

impl GhostUi {
    pub(crate) fn workflow_view(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        if ui.available_width() < 950.0 {
            egui::ScrollArea::vertical()
                .id_salt("workflow_controls_scroll")
                .show(ui, |ui| {
                    self.capture_card(ui, state);
                    ui.add_space(8.0);
                    self.intent_card(ui, state);
                    ui.add_space(8.0);
                    self.stage_actions(ui, state, session);
                    ui.add_space(8.0);
                    self.graph_summary(ui, state);
                    ui.add_space(8.0);
                    panel_frame().show(ui, |ui| {
                        ui.label(section("PROPOSAL PREVIEW"));
                        widgets::proposal(
                            ui,
                            session.proposal.as_ref().map(|preview| &preview.plan),
                        );
                    });
                });
        } else {
            ui.columns(2, |columns| {
                egui::ScrollArea::vertical()
                    .id_salt("workflow_controls_scroll")
                    .show(&mut columns[0], |ui| {
                        self.capture_card(ui, state);
                        ui.add_space(8.0);
                        self.intent_card(ui, state);
                        ui.add_space(8.0);
                        self.stage_actions(ui, state, session);
                        ui.add_space(8.0);
                        self.graph_summary(ui, state);
                    });
                panel_frame().show(&mut columns[1], |ui| {
                    ui.label(section("PROPOSAL PREVIEW"));
                    egui::ScrollArea::vertical()
                        .id_salt("workflow_proposal_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            widgets::proposal(
                                ui,
                                session.proposal.as_ref().map(|preview| &preview.plan),
                            )
                        });
                });
            });
        }
    }

    pub(crate) fn analyzer_view(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.label(section("CAPTURE TAP"));
            for tap in state.graph.taps() {
                let response = ui.add(egui::Button::selectable(
                    state.selected_tap == tap.id,
                    &tap.label,
                ));
                if response.clicked() {
                    state.selected_tap = tap.id;
                    session.analysis = None;
                    session.proposal = None;
                }
                response.on_hover_text("The next DAW capture records this graph edge");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        session.captured.is_some() && session.analysis_receiver.is_none(),
                        egui::Button::new("ANALYZE").fill(accent()),
                    )
                    .clicked()
                {
                    self.start_analysis(state, session);
                }
            });
        });
        ui.add_space(8.0);
        widgets::signal_field(ui, session.analysis.as_ref());
        ui.add_space(8.0);
        if let Some(result) = &session.analysis {
            widgets::metrics(ui, &result.bundle);
            if !result.bundle.signal.flags.is_empty() {
                ui.add_space(6.0);
                for flag in &result.bundle.signal.flags {
                    ui.colored_label(orange(), format!("{} · {}", flag.severity, flag.message));
                }
            }
        }
    }

    pub(crate) fn routing_view(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        if ui.available_width() < 950.0 {
            egui::ScrollArea::vertical()
                .id_salt("routing_graph_scroll")
                .show(ui, |ui| {
                    self.graph_editor(ui, state, session);
                    ui.add_space(12.0);
                    self.discovery_panel(ui, state, session);
                });
        } else {
            ui.columns(2, |columns| {
                egui::ScrollArea::vertical()
                    .id_salt("routing_graph_scroll")
                    .show(&mut columns[0], |ui| self.graph_editor(ui, state, session));
                self.discovery_panel(&mut columns[1], state, session);
            });
        }
    }

    pub(crate) fn capture_card(&mut self, ui: &mut egui::Ui, state: &mut PersistedUiState) {
        panel_frame().show(ui, |ui| {
            ui.label(section("CAPTURE SOURCE"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.capture_source, CaptureSource::Daw, "DAW audio");
                ui.selectable_value(&mut state.capture_source, CaptureSource::File, "Media file");
            });
            match state.capture_source {
                CaptureSource::Daw => {
                    let config = self.daw.audio_configuration();
                    ui.horizontal(|ui| {
                        ui.label("Length");
                        ui.add(
                            egui::Slider::new(&mut state.capture_seconds, 0.5..=24.0)
                                .suffix(" s")
                                .logarithmic(true),
                        );
                    });
                    if let Some(rate) = config.sample_rate {
                        ui.label(
                            egui::RichText::new(format!(
                                "Host: {rate:.0} Hz · up to {} frames/block",
                                config.maximum_frames.unwrap_or_default()
                            ))
                            .small()
                            .color(blue()),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Waiting for DAW activation")
                                .small()
                                .color(orange()),
                        );
                    }
                    let (written, target) = self.capture.progress();
                    if target > 0 && self.capture.state() == RealtimeCaptureState::Recording {
                        ui.add(
                            egui::ProgressBar::new(written as f32 / target as f32)
                                .text(format!("Recording {written}/{target} frames")),
                        );
                    }
                }
                CaptureSource::File => {
                    ui.label("MEDIA PATH");
                    ui.text_edit_singleline(&mut state.input_path);
                }
            }
        });
    }

    pub(crate) fn intent_card(&self, ui: &mut egui::Ui, state: &mut PersistedUiState) {
        panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(section("MIX INTENT"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("analysis_quality")
                        .selected_text(["Live", "High", "Maximum"][state.profile])
                        .show_ui(ui, |ui| {
                            for (index, label) in ["Live", "High", "Maximum"].iter().enumerate() {
                                ui.selectable_value(&mut state.profile, index, *label);
                            }
                        });
                });
            });
            ui.add(egui::TextEdit::multiline(&mut state.prompt).desired_rows(4));
        });
    }

    pub(crate) fn stage_actions(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        ui.horizontal_wrapped(|ui| {
            let capture_label = if session.active_stage == Some(Stage::Capture) {
                "CAPTURING…"
            } else {
                "1  CAPTURE"
            };
            if ui
                .add_enabled(
                    session.capture_receiver.is_none()
                        && self.capture.state() != RealtimeCaptureState::Recording,
                    egui::Button::new(capture_label).min_size(egui::vec2(120.0, 38.0)),
                )
                .clicked()
            {
                self.start_capture(state, session);
            }
            let analysis_label = if session.active_stage == Some(Stage::Analyze) {
                "ANALYZING…"
            } else {
                "2  ANALYZE"
            };
            if ui
                .add_enabled(
                    session.captured.is_some() && session.analysis_receiver.is_none(),
                    egui::Button::new(analysis_label).min_size(egui::vec2(120.0, 38.0)),
                )
                .clicked()
            {
                self.start_analysis(state, session);
            }
            let propose_label = if session.active_stage == Some(Stage::Propose) {
                "PROPOSING…"
            } else {
                "3  PROPOSE"
            };
            if ui
                .add_enabled(
                    session.analysis.is_some() && session.proposal_receiver.is_none(),
                    egui::Button::new(propose_label)
                        .fill(accent())
                        .min_size(egui::vec2(120.0, 38.0)),
                )
                .clicked()
            {
                self.start_proposal(state, session);
            }
            let can_apply = session
                .proposal
                .as_ref()
                .is_some_and(|preview| preview.patch.can_apply())
                && !matches!(
                    session.patch_state,
                    crate::PatchApplicationState::Queued { .. }
                );
            if ui
                .add_enabled(can_apply, egui::Button::new("4  APPLY"))
                .on_hover_text(
                    "Apply the complete reviewed mapping as one revision-bound transaction",
                )
                .clicked()
            {
                self.apply_preview(state, session);
            }
            if ui
                .add_enabled(
                    session.last_applied_patch.is_some()
                        && !matches!(
                            session.patch_state,
                            crate::PatchApplicationState::Queued { .. }
                        ),
                    egui::Button::new("UNDO"),
                )
                .clicked()
            {
                self.undo_last_patch(state, session);
            }
        });
        if let Some(preview) = &session.proposal {
            if preview.patch.mapping_issues.is_empty() {
                ui.label(
                    egui::RichText::new(format!(
                        "Mapped {} parameters · graph r{} · {:?}",
                        preview.patch.parameter_changes.len(),
                        preview.patch.expected_graph_revision,
                        session.patch_state
                    ))
                    .small()
                    .color(accent()),
                );
                egui::CollapsingHeader::new("REVIEW CONCRETE PATCH")
                    .default_open(true)
                    .show(ui, |ui| {
                        for change in &preview.patch.parameter_changes {
                            let mapping = format!(
                                "{} → {}:{} = {:.3} (was {:.3}) · {:.0}% mapping confidence",
                                change.semantic_field,
                                change.target_node_id,
                                change.parameter_id,
                                change.plain_value,
                                change.previous_value,
                                change.mapping_confidence * 100.0
                            );
                            ui.add(egui::Label::new(&mapping).truncate())
                                .on_hover_text(mapping);
                        }
                        for change in &preview.patch.bypass_changes {
                            ui.label(format!(
                                "bypass → {} = {} (was {})",
                                change.target_node_id, change.bypassed, change.previous_bypassed
                            ));
                        }
                    });
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "Mapping incomplete · {} issues · Apply disabled",
                        preview.patch.mapping_issues.len()
                    ))
                    .small()
                    .color(orange()),
                );
                for issue in preview.patch.mapping_issues.iter().take(6) {
                    ui.label(egui::RichText::new(format!("• {issue}")).small().weak());
                }
            }
        }
    }

    pub(crate) fn graph_summary(&self, ui: &mut egui::Ui, state: &mut PersistedUiState) {
        panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(section("PROCESSOR GRAPH"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("EDIT IN ROUTING →").clicked() {
                        state.selected_view = 2;
                    }
                });
            });
            if state.graph.nodes.is_empty() {
                ui.label("Empty graph — analysis remains available; proposals need a typed node.");
            }
            for (index, node) in state.graph.nodes.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if node.bypassed { orange() } else { accent() },
                        format!("{:02}", index + 1),
                    );
                    ui.strong(node.display_name());
                    ui.label(egui::RichText::new(node.class.label()).small().weak());
                });
            }
        });
    }

    pub(crate) fn graph_editor(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        ui.horizontal(|ui| {
            ui.label(section("GRAPH PRESET"));
            for preset in GraphPreset::ALL {
                if ui.small_button(preset.label()).clicked() {
                    state.graph.apply_preset(preset);
                    session.selected_node_id = None;
                    session.analysis = None;
                    session.proposal = None;
                    self.graph_commit_requested = true;
                }
            }
        });
        ui.add_space(7.0);
        enum NodeAction {
            Remove(usize),
            Move(usize, isize),
            Select(usize),
            Assign(usize),
        }
        let mut action = None;
        for (index, node) in state.graph.nodes.iter_mut().enumerate() {
            let selected = session.selected_node_id.as_deref() == Some(node.id.as_str());
            egui::Frame::NONE
                .fill(if selected {
                    egui::Color32::from_rgb(20, 38, 45)
                } else {
                    egui::Color32::from_rgb(13, 20, 29)
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if selected {
                        accent()
                    } else {
                        egui::Color32::from_gray(35)
                    },
                ))
                .corner_radius(8.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(selected, format!("{:02}", index + 1))
                            .clicked()
                        {
                            action = Some(NodeAction::Select(index));
                        }
                        let previous_class = node.class;
                        egui::ComboBox::from_id_salt(("node_class", &node.id))
                            .selected_text(node.class.label())
                            .show_ui(ui, |ui| {
                                for class in ProcessorClass::ALL {
                                    ui.selectable_value(&mut node.class, class, class.label());
                                }
                            });
                        if node.class != previous_class {
                            self.graph_commit_requested = true;
                        }
                        let bypass_label = if node.bypassed { "BYP" } else { "ON" };
                        if ui.toggle_value(&mut node.bypassed, bypass_label).changed() {
                            self.document_dirty_requested = true;
                        }
                        if ui.small_button("↑").clicked() {
                            action = Some(NodeAction::Move(index, -1));
                        }
                        if ui.small_button("↓").clicked() {
                            action = Some(NodeAction::Move(index, 1));
                        }
                        if ui.small_button("REMOVE").clicked() {
                            action = Some(NodeAction::Remove(index));
                        }
                    });
                    ui.horizontal(|ui| {
                        let plugin_name = node
                            .plugin
                            .as_ref()
                            .map_or("No child assigned", |plugin| plugin.name.as_str());
                        ui.add(egui::Label::new(plugin_name).truncate())
                            .on_hover_text(plugin_name);
                        if session.selected_plugin.is_some()
                            && ui.small_button("USE SELECTED CLAP").clicked()
                        {
                            action = Some(NodeAction::Assign(index));
                        }
                        if node.plugin.is_some() {
                            if ui.small_button("OPEN UI").clicked() {
                                self.host_control.show_child_gui(&node.id);
                            }
                            if ui.small_button("HIDE UI").clicked() {
                                self.host_control.hide_child_gui(&node.id);
                            }
                        }
                    });
                    if !node.class.context_available() {
                        ui.label(
                            egui::RichText::new(
                                "Context recipe planned; EQ and compressor are available now",
                            )
                            .small()
                            .color(orange()),
                        );
                    }
                    if selected {
                        if let Some(plugin) = &node.plugin {
                            egui::CollapsingHeader::new(format!(
                                "PUBLIC PARAMETERS ({})",
                                plugin.public_parameters.len()
                            ))
                            .id_salt(("parameter_manifest", &node.id))
                            .show(ui, |ui| {
                                if plugin.public_parameters.is_empty() {
                                    ui.label("The child exposes no clap.params manifest.");
                                }
                                egui::ScrollArea::horizontal().show(ui, |ui| {
                                    egui::Grid::new(("parameter_grid", &node.id))
                                        .num_columns(3)
                                        .striped(true)
                                        .show(ui, |ui| {
                                            for parameter in
                                                plugin.public_parameters.iter().take(96)
                                            {
                                                ui.add(
                                                    egui::Label::new(&parameter.name).truncate(),
                                                )
                                                .on_hover_text(&parameter.name);
                                                let module = parameter
                                                    .module
                                                    .as_deref()
                                                    .unwrap_or("General");
                                                ui.add(egui::Label::new(module).truncate())
                                                    .on_hover_text(module);
                                                let range = format!(
                                                    "{:.2} … {:.2}",
                                                    parameter.minimum, parameter.maximum
                                                );
                                                ui.add(egui::Label::new(&range).truncate())
                                                    .on_hover_text(range);
                                                ui.end_row();
                                            }
                                        });
                                });
                            });
                        }
                    }
                });
            ui.add_space(6.0);
        }
        if let Some(action) = action {
            match action {
                NodeAction::Remove(index) => {
                    state.graph.remove(index);
                    session.selected_node_id = None;
                    self.graph_commit_requested = true;
                }
                NodeAction::Move(index, delta) => {
                    if state.graph.move_by(index, delta) {
                        let moved = (index as isize + delta) as usize;
                        session.selected_node_id =
                            state.graph.nodes.get(moved).map(|node| node.id.clone());
                        self.graph_commit_requested = true;
                    }
                }
                NodeAction::Select(index) => {
                    session.selected_node_id =
                        state.graph.nodes.get(index).map(|node| node.id.clone());
                }
                NodeAction::Assign(index) => self.assign_selected_plugin(state, session, index),
            }
        }
        if ui.button("＋  CREATE NODE").clicked() {
            let id = format!("node-{}", state.next_node_id);
            state.next_node_id += 1;
            state.graph.create_node(id, ProcessorClass::Equalizer);
            session.selected_node_id = state.graph.nodes.last().map(|node| node.id.clone());
            self.graph_commit_requested = true;
        }
        ui.add_space(10.0);
        ui.label(section("GRAPH-DERIVED CAPTURE TAPS"));
        for tap in state.graph.taps() {
            ui.horizontal(|ui| {
                let selected = state.selected_tap == tap.id;
                if ui.add(egui::Button::selectable(selected, "●")).clicked() {
                    state.selected_tap = tap.id.clone();
                }
                ui.label(&tap.label);
                ui.label(egui::RichText::new("recordable").small().weak());
            });
        }
    }

    pub(crate) fn discovery_panel(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    session.scan_receiver.is_none(),
                    egui::Button::new(if state.scanner_open {
                        "HIDE DISCOVERY"
                    } else {
                        "DISCOVER CLAP"
                    }),
                )
                .clicked()
            {
                if state.scanner_open {
                    state.scanner_open = false;
                } else {
                    state.scanner_open = true;
                    if session.plugins.is_empty() {
                        self.start_scan(session);
                    }
                }
            }
            if state.scanner_open && ui.small_button("RESCAN").clicked() {
                self.start_scan(session);
            }
        });
        if !state.scanner_open {
            panel_frame().show(ui, |ui| {
                ui.label("Discovery is hidden. The graph remains fully editable.");
            });
            return;
        }
        ui.label(
            egui::RichText::new(format!(
                "{} processors · {} scan errors",
                session.plugins.len(),
                session.scan_errors
            ))
            .small()
            .weak(),
        );
        egui::ScrollArea::vertical()
            .id_salt("clap_scanner_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for plugin in &session.plugins {
                    let selected = session
                        .selected_plugin
                        .as_ref()
                        .is_some_and(|identity| identity.matches(plugin));
                    let label = format!(
                        "{}  ·  {}",
                        plugin.name,
                        plugin.vendor.as_deref().unwrap_or("Unknown vendor")
                    );
                    if ui
                        .add(
                            egui::Label::new(egui::RichText::new(&label).color(if selected {
                                accent()
                            } else {
                                ui.visuals().text_color()
                            }))
                            .truncate()
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text(plugin.path.display().to_string())
                        .clicked()
                    {
                        session.selected_plugin =
                            Some(crate::PluginIdentity::new(&plugin.path, &plugin.id));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{} public parameters",
                            plugin.public_parameters.len()
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
    }
}
