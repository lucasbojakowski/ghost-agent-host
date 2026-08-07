use eframe::egui;
use ghost_ui::{GhostUi, DEFAULT_EDITOR_HEIGHT, DEFAULT_EDITOR_WIDTH};

#[derive(Default)]
struct GhostLabApp {
    ui: GhostUi,
}

impl eframe::App for GhostLabApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui.show(ui);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([DEFAULT_EDITOR_WIDTH as f32, DEFAULT_EDITOR_HEIGHT as f32])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Ghost Agent Host Lab",
        options,
        Box::new(|_| Ok(Box::new(GhostLabApp::default()))),
    )
}
