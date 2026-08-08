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
    let height = ui.available_height().clamp(220.0, 390.0);
    let desired = egui::vec2(ui.available_width(), height);
    let (response, painter) = ui.allocate_painter(desired, egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 9.0, egui::Color32::from_rgb(10, 17, 26));
    for index in 0..=8 {
        let x = egui::lerp(rect.x_range(), index as f32 / 8.0);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_gray(27)),
        );
    }
    for index in 0..=4 {
        let y = egui::lerp(rect.y_range(), index as f32 / 4.0);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(24)),
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
    if result.spectrum.len() > 1 {
        let points: Vec<_> = result
            .spectrum
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let normalized_x = index as f32 / (result.spectrum.len() - 1) as f32;
                let x = egui::lerp(rect.x_range(), normalized_x.powf(0.72));
                let shaped = 0.5 + (*value - 0.5) * 0.72;
                egui::pos2(x, egui::lerp(rect.y_range(), 1.0 - shaped))
            })
            .collect();
        for width in [8.0, 4.0] {
            painter.add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(width, accent().gamma_multiply(0.08)),
            ));
        }
        painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, accent())));
    }
    painter.text(
        rect.left_top() + egui::vec2(12.0, 10.0),
        egui::Align2::LEFT_TOP,
        format!("{} · {}", result.tap_label, result.source_label),
        egui::FontId::monospace(11.0),
        egui::Color32::from_gray(130),
    );
}

pub(crate) fn metrics(ui: &mut egui::Ui, analysis: &AnalysisBundle) {
    let signal = &analysis.signal;
    egui::Grid::new("analysis_metrics")
        .num_columns(4)
        .spacing([18.0, 5.0])
        .show(ui, |ui| {
            metric(ui, "RMS", signal.loudness.rms_dbfs, "dBFS");
            metric(ui, "CREST", signal.loudness.crest_factor_db, "dB");
            metric(ui, "CENTROID", signal.spectrum.centroid_hz, "Hz");
            metric(ui, "LOW MID", signal.spectrum.bands.low_mid_db, "dB");
        });
}

fn metric(ui: &mut egui::Ui, label: &str, value: f64, unit: &str) {
    ui.vertical(|ui| {
        ui.label(section(label));
        ui.label(egui::RichText::new(format!("{value:.1} {unit}")).size(15.0));
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
