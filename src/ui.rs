use crate::recorder::{Recorder, RecorderState};
use eframe::egui::{self, Color32, FontId, RichText, Vec2, ViewportCommand};

pub struct RecMiApp {
    recorder: Recorder,
    error_msg: Option<String>,
    was_recording: bool,
}

impl RecMiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            recorder: Recorder::new(),
            error_msg: None,
            was_recording: false,
        }
    }
}

impl eframe::App for RecMiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_recording = self.recorder.is_recording();

        // Handle window minimization lifecycle (M7)
        if is_recording && !self.was_recording {
            // Minimize window when recording begins so recorder is hidden from video
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        } else if !is_recording && self.was_recording {
            // Restore and bring window to focus when recording stops
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        self.was_recording = is_recording;

        // Request continuous UI repaint while recording to animate live timer
        if is_recording {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);

                // Title Header
                ui.label(
                    RichText::new("RecMi")
                        .font(FontId::proportional(22.0))
                        .strong()
                        .color(Color32::from_rgb(240, 240, 245)),
                );

                ui.add_space(16.0);

                if is_recording {
                    // --- RECORDING STATE ---
                    let elapsed_secs = self.recorder.elapsed_secs();
                    let mins = elapsed_secs / 60;
                    let secs = elapsed_secs % 60;
                    let time_str = format!("{:02}:{:02}", mins, secs);

                    // Live Timer Display
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 110.0) / 2.0);
                        ui.label(
                            RichText::new("●")
                                .font(FontId::proportional(18.0))
                                .color(Color32::from_rgb(235, 59, 90)),
                        );
                        ui.label(
                            RichText::new(&time_str)
                                .font(FontId::monospace(26.0))
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });

                    ui.add_space(18.0);

                    // STOP Button
                    let stop_btn = egui::Button::new(
                        RichText::new("STOP")
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(235, 59, 90))
                    .min_size(Vec2::new(160.0, 42.0));

                    if ui.add(stop_btn).clicked() {
                        self.recorder.stop();
                    }
                } else {
                    // --- IDLE STATE ---
                    let record_btn = egui::Button::new(
                        RichText::new("● RECORD")
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(229, 9, 20))
                    .min_size(Vec2::new(180.0, 44.0));

                    if ui.add(record_btn).clicked() {
                        self.error_msg = None;
                        if let Err(err) = self.recorder.start() {
                            self.error_msg = Some(err);
                        }
                    }

                    ui.add_space(14.0);

                    // Subtitle / Specs
                    ui.label(
                        RichText::new("30 FPS  ·  MP4")
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(140, 145, 155)),
                    );
                }

                ui.add_space(12.0);

                // Notifications / Messages
                if let Some(err) = &self.error_msg {
                    ui.label(
                        RichText::new(err)
                            .font(FontId::proportional(12.0))
                            .color(Color32::LIGHT_RED),
                    );
                } else if let RecorderState::Finished { saved_path } = self.recorder.state() {
                    ui.label(
                        RichText::new(format!("Saved: {}", saved_path))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(46, 213, 115)),
                    );
                }
            });
        });
    }
}
