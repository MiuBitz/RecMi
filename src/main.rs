use eframe::egui;

mod recorder;
mod ui;

use ui::RecMiApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 220.0])
            .with_min_inner_size([340.0, 220.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "RecMi",
        native_options,
        Box::new(|cc| Ok(Box::new(RecMiApp::new(cc)))),
    )
}
