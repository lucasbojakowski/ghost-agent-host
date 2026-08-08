use ghost_core::AnalysisBundle;
use ghost_mix::{MixOperation, MixPlan};

use crate::work::AnalysisResult;

pub(crate) fn configure_style(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(8, 12, 18);
    visuals.window_fill = egui::Color32::from_rgb(11, 16, 23);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(18, 26, 36);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(28, 42, 55);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(42, 61, 73);
    visuals.selection.bg_fill = accent().gamma_multiply(0.35);
    context.set_visuals(visuals);
}

pub(crate) fn accent() -> egui::Color32 {
    egui::Color32::from_rgb(91, 224, 205)
}

pub(crate) fn orange() -> egui::Color32 {
    egui::Color32::from_rgb(255, 174, 91)
}

pub(crate) fn blue() -> egui::Color32 {
    egui::Color32::from_rgb(90, 190, 255)
}

pub(crate) fn section(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(egui::Color32::from_gray(150))
}

pub(crate) fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(12, 18, 27))
        .corner_radius(8.0)
        .inner_margin(12.0)
}

pub(crate) fn signal_field(ui: &mut egui::Ui, result: Option<&AnalysisResult>) {
    const MIN_HZ: f32 = 20.0;
    const MAX_HZ: f32 = 20_000.0;
    const MIN_DB: f32 = -72.0;
    const MAX_DB: f32 = 0.0;

    let height = ui.available_height().clamp(260.0, 430.0);
    let desired = egui::vec2(ui.available_width(), height);
    let (response, painter) = ui.allocate_painter(desired, egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 9.0, egui::Color32::from_rgb(10, 17, 26));
    let graph = rect.shrink2(egui::vec2(42.0, 28.0));

    let x_for_hz = |hz: f32| {
        let normalized = (hz.clamp(MIN_HZ, MAX_HZ).ln() - MIN_HZ.ln())
            / (MAX_HZ.ln() - MIN_HZ.ln());
        egui::lerp(graph.x_range(), normalized)
    };
    let y_for_db = |db: f32| {
        let normalized = ((db.clamp(MIN_DB, MAX_DB) - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0);
        egui::lerp(graph.y_range(), 1.0 - normalized)
    };

    for frequency in [20.0_f32, 50.0, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0, 20_000.0] {
        let x = x_for_hz(frequency);
        painter.line_segment(
            [egui::pos2(x, graph.top()), egui::pos2(x, graph.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_gray(27)),
        );
        let label = if frequency >= 1_000.0 {
            format!("{}k", (frequency / 1_000.0) as i32)
        } else {
            format!("{}", frequency as i32)
        };
        painter.text(
            egui::pos2(x, graph.bottom() + 7.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::monospace(9.0),
            egui::Color32::from_gray(90),
        );
    }
    for db in [-72.0_f32, -54.0, -36.0, -18.0, 0.0] {
        let y = y_for_db(db);
        painter.line_segment(
            [egui::pos2(graph.left(), y), egui::pos2(graph.right(), y)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(24)),
        );
        painter.text(
            egui::pos2(graph.left() - 7.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{db:.0}"),
            egui::FontId::monospace(9.0),
            egui::Color32::from_gray(88),
        );
    }

    let Some(result) = result else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Capture and analyze a signal to populate this view",
            egui::FontId::proportional(13.0),
            egui::Color32::from_gray(105),
        );
        return;
    };

    let spectral = &result.bundle.signal.spectrum;
    for resonance in spectral.resonances.iter().take(8).rev() {
        let half_bandwidth = resonance.bandwidth_octaves as f32 * 0.5;
        let center = resonance.frequency_hz as f32;
        let low = center / 2.0_f32.powf(half_bandwidth);
        let high = center * 2.0_f32.powf(half_bandwidth);
        let left = x_for_hz(low);
        let right = x_for_hz(high);
        let strength = (resonance.persistence as f32).clamp(0.08, 1.0);
        let region = egui::Rect::from_min_max(
            egui::pos2(left, graph.top()),
            egui::pos2(right.max(left + 1.0), graph.bottom()),
        );
        painter.rect_filled(region, 0.0, orange().gamma_multiply(0.035 + 0.075 * strength));
    }

    if spectral.display_spectrum.len() > 1 {
        let points: Vec<_> = spectral
            .display_spectrum
            .iter()
            .map(|point| {
                egui::pos2(
                    x_for_hz(point.frequency_hz),
                    y_for_db(point.magnitude_db),
                )
            })
            .collect();
        for width in [8.0, 4.0] {
            painter.add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(width, accent().gamma_multiply(0.07)),
            ));
        }
        painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, accent())));
    }

    for (frequency, label, color) in [
        (spectral.centroid_hz as f32, "C", blue()),
        (spectral.rolloff_85_hz as f32, "R85", orange()),
    ] {
        if frequency.is_finite() && (MIN_HZ..=MAX_HZ).contains(&frequency) {
            let x = x_for_hz(frequency);
            painter.line_segment(
                [egui::pos2(x, graph.top()), egui::pos2(x, graph.bottom())],
                egui::Stroke::new(1.0, color.gamma_multiply(0.55)),
            );
            painter.text(
                egui::pos2(x + 4.0, graph.top() + 5.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::monospace(9.0),
                color,
            );
        }
    }

    for resonance in spectral
        .resonances
        .iter()
        .filter(|candidate| candidate.persistence >= 0.12)
        .take(5)
    {
        let x = x_for_hz(resonance.frequency_hz as f32);
        painter.circle_filled(egui::pos2(x, graph.top() + 18.0), 2.5, orange());
        painter.text(
            egui::pos2(x, graph.top() + 25.0),
            egui::Align2::CENTER_TOP,
            format!(
                "{:.0} Hz\n+{:.1} dB · {:.0}%",
                resonance.frequency_hz,
                resonance.prominence_db,
                resonance.persistence * 100.0
            ),
            egui::FontId::monospace(8.0),
            egui::Color32::from_gray(150),
        );
    }

    painter.text(
        rect.left_top() + egui::vec2(12.0, 9.0),
        egui::Align2::LEFT_TOP,
        format!("{} · {}", result.tap_label, result.source_label),
        egui::FontId::monospace(10.0),
        egui::Color32::from_gray(130),
    );
}

pub(crate) fn metrics(ui: &mut egui::Ui, analysis: &AnalysisBundle) {
    let signal = &analysis.signal;
    egui::Grid::new("analysis_metrics")
        .num_columns(4)
        .spacing([18.0, 7.0])
        .show(ui, |ui| {
            optional_metric(ui, "LUFS", signal.loudness.integrated_lufs, "LUFS");
            metric(ui, "RMS", signal.loudness.rms_dbfs, "dBFS");
            metric(ui, "PEAK", signal.integrity.sample_peak_dbfs, "dBFS");
            metric(ui, "CREST", signal.loudness.crest_factor_db, "dB");
            ui.end_row();
            metric(ui, "CENTROID", signal.spectrum.centroid_hz, "Hz");
            metric(ui, "ROLLOFF 85", signal.spectrum.rolloff_85_hz, "Hz");
            metric(ui, "TILT", signal.spectrum.tilt_db_per_octave, "dB/oct");
            metric(ui, "CORRELATION", signal.stereo.broadband_correlation, "");
            ui.end_row();
        });
}

fn metric(ui: &mut egui::Ui, label: &str, value: f64, unit: &str) {
    ui.vertical(|ui| {
        ui.label(section(label));
        let rendered = if unit.is_empty() {
            format!("{value:.2}")
        } else {
            format!("{value:.1} {unit}")
        };
        ui.label(egui::RichText::new(rendered).size(15.0));
    });
}

fn optional_metric(ui: &mut egui::Ui, label: &str, value: Option<f64>, unit: &str) {
    ui.vertical(|ui| {
        ui.label(section(label));
        ui.label(
            egui::RichText::new(
                value.map_or_else(|| "—".to_owned(), |value| format!("{value:.1} {unit}")),
            )
            .size(15.0),
        );
    });
}

pub(crate) fn proposal(ui: &mut egui::Ui, plan: Option<&MixPlan>) {
    let Some(plan) = plan else {
        ui.label(
            egui::RichText::new("No proposal yet. Capture, analyze, then request a proposal.")
                .color(egui::Color32::from_gray(115)),
        );
        return;
    };
    ui.vertical(|ui| {
        ui.heading(&plan.summary);
        ui.label(
            egui::RichText::new(format!("{:.0}% confidence", plan.confidence * 100.0))
                .color(accent()),
        );
    });
    ui.add_space(8.0);
    ui.label(section("PROPOSED CHANGES"));
    if plan.operations.is_empty() {
        ui.label("No processing change is justified by the captured evidence.");
    }
    for (index, operation) in plan.operations.iter().enumerate() {
        panel_frame().show(ui, |ui| match operation {
            MixOperation::EqBand { settings } => {
                ui.strong(format!(
                    "{}. EQ · {:?} at {:.0} Hz",
                    index + 1,
                    settings.shape,
                    settings.frequency_hz
                ));
                ui.label(format!(
                    "{:+.1} dB · Q {:.2} · {}",
                    settings.gain_db, settings.q, settings.channel_mode
                ));
                ui.label(egui::RichText::new(&settings.rationale).weak());
                evidence(ui, &settings.evidence);
            }
            MixOperation::Compressor { settings } => {
                ui.strong(format!("{}. Compressor · {}", index + 1, settings.style));
                ui.label(format!(
                    "Threshold {:.1} dB · {:.1}:1 · attack {:.1} ms · release {:.0} ms",
                    settings.threshold_db, settings.ratio, settings.attack_ms, settings.release_ms
                ));
                ui.label(egui::RichText::new(&settings.rationale).weak());
                evidence(ui, &settings.evidence);
            }
            MixOperation::Bypass { target, bypassed } => {
                ui.strong(format!(
                    "{}. {:?} {}",
                    index + 1,
                    target,
                    if *bypassed { "bypassed" } else { "enabled" }
                ));
            }
        });
        ui.add_space(5.0);
    }
    if !plan.expected_changes.is_empty() {
        ui.label(section("EXPECTED"));
        for item in &plan.expected_changes {
            ui.label(format!("• {} — {}", item.metric, item.direction));
        }
    }
    if !plan.cautions.is_empty() {
        ui.add_space(6.0);
        ui.label(section("LISTEN FOR"));
        for caution in &plan.cautions {
            ui.colored_label(orange(), format!("• {caution}"));
        }
    }
}

fn evidence(ui: &mut egui::Ui, items: &[String]) {
    for item in items.iter().take(3) {
        ui.label(
            egui::RichText::new(format!("Evidence · {item}"))
                .small()
                .color(blue()),
        );
    }
}
