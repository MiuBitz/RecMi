use crate::recorder::{Recorder, RecorderState, RecordingRegion};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2, ViewportCommand};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectedMode {
    Fullscreen,
    Region,
}

pub struct RecMiApp {
    recorder: Recorder,
    mode: SelectedMode,
    region: Option<RecordingRegion>,
    is_selecting_region: bool,
    drag_start: Option<Pos2>,
    drag_current: Option<Pos2>,
    error_msg: Option<String>,
    was_recording: bool,
}

impl RecMiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            recorder: Recorder::new(),
            mode: SelectedMode::Fullscreen,
            region: None,
            is_selecting_region: false,
            drag_start: None,
            drag_current: None,
            error_msg: None,
            was_recording: false,
        }
    }

    fn start_region_selection(&mut self, ctx: &egui::Context) {
        self.is_selecting_region = true;
        self.drag_start = None;
        self.drag_current = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn finish_region_selection(&mut self, ctx: &egui::Context) {
        if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
            let min_x = start.x.min(current.x).max(0.0) as u32;
            let min_y = start.y.min(current.y).max(0.0) as u32;
            let max_x = start.x.max(current.x).max(0.0) as u32;
            let max_y = start.y.max(current.y).max(0.0) as u32;

            let w = max_x.saturating_sub(min_x);
            let h = max_y.saturating_sub(min_y);

            if w >= 20 && h >= 20 {
                self.region = Some(RecordingRegion::new(min_x, min_y, w, h));
                self.mode = SelectedMode::Region;
            }
        }

        self.is_selecting_region = false;
        self.drag_start = None;
        self.drag_current = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(340.0, 240.0)));
    }

    fn cancel_region_selection(&mut self, ctx: &egui::Context) {
        self.is_selecting_region = false;
        self.drag_start = None;
        self.drag_current = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(340.0, 240.0)));
    }
}

impl eframe::App for RecMiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 1. REGION SELECTION OVERLAY ---
        if self.is_selecting_region {
            // Cancel on Escape
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel_region_selection(ctx);
                return;
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_black_alpha(80)))
                .show(ctx, |ui| {
                    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());

                    if response.drag_started() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_start = Some(pos);
                            self.drag_current = Some(pos);
                        }
                    }

                    if response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_current = Some(pos);
                        }
                    }

                    if response.drag_stopped() || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.finish_region_selection(ctx);
                        return;
                    }

                    // Render Selection Box & Dimensions Badge
                    if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
                        let rect = Rect::from_two_pos(start, current);
                        
                        // Clear cutout border
                        painter.rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(2.5f32, Color32::from_rgb(229, 9, 20)),
                        );

                        // Dimensions Label Badge
                        let w = (rect.width().abs() as u32) & !1;
                        let h = (rect.height().abs() as u32) & !1;
                        let badge_text = format!(" {} × {} px ", w, h);

                        let badge_pos = Pos2::new(
                            rect.left().max(10.0),
                            (rect.top() - 28.0).max(10.0),
                        );

                        painter.text(
                            badge_pos,
                            Align2::LEFT_TOP,
                            badge_text,
                            FontId::monospace(14.0),
                            Color32::WHITE,
                        );
                    } else {
                        // Instruction overlay when not dragging yet
                        painter.text(
                            ui.available_rect_before_wrap().center(),
                            Align2::CENTER_CENTER,
                            "Click and drag to select recording area\nPress Esc to cancel",
                            FontId::proportional(20.0),
                            Color32::WHITE,
                        );
                    }
                });

            ctx.request_repaint();
            return;
        }

        // --- 2. MAIN APPLICATION LIFECYCLE ---
        let is_recording = self.recorder.is_recording();

        if is_recording && !self.was_recording {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        } else if !is_recording && self.was_recording {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        self.was_recording = is_recording;

        if is_recording {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);

                // Title Header
                ui.label(
                    RichText::new("RecMi")
                        .font(FontId::proportional(22.0))
                        .strong()
                        .color(Color32::from_rgb(240, 240, 245)),
                );

                ui.add_space(12.0);

                if is_recording {
                    // --- RECORDING STATE ---
                    let elapsed_secs = self.recorder.elapsed_secs();
                    let mins = elapsed_secs / 60;
                    let secs = elapsed_secs % 60;
                    let time_str = format!("{:02}:{:02}", mins, secs);

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

                    ui.add_space(14.0);

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
                    // Mode Switcher (Fullscreen / Region)
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 230.0) / 2.0);

                        let fs_color = if self.mode == SelectedMode::Fullscreen {
                            Color32::from_rgb(229, 9, 20)
                        } else {
                            Color32::from_rgb(45, 50, 60)
                        };

                        if ui.add(egui::Button::new(
                            RichText::new("🖥 Fullscreen").font(FontId::proportional(12.0)).color(Color32::WHITE)
                        ).fill(fs_color).min_size(Vec2::new(110.0, 26.0))).clicked() {
                            self.mode = SelectedMode::Fullscreen;
                        }

                        let reg_color = if self.mode == SelectedMode::Region {
                            Color32::from_rgb(229, 9, 20)
                        } else {
                            Color32::from_rgb(45, 50, 60)
                        };

                        if ui.add(egui::Button::new(
                            RichText::new("✂ Region").font(FontId::proportional(12.0)).color(Color32::WHITE)
                        ).fill(reg_color).min_size(Vec2::new(110.0, 26.0))).clicked() {
                            self.mode = SelectedMode::Region;
                            if self.region.is_none() {
                                self.start_region_selection(ctx);
                            }
                        }
                    });

                    ui.add_space(10.0);

                    // If in Region mode, show Select Area button / region dimensions
                    if self.mode == SelectedMode::Region {
                        if let Some(r) = self.region {
                            ui.horizontal(|ui| {
                                ui.add_space((ui.available_width() - 200.0) / 2.0);
                                ui.label(
                                    RichText::new(format!("Area: {}×{} px", r.width, r.height))
                                        .font(FontId::proportional(12.0))
                                        .color(Color32::from_rgb(180, 185, 195)),
                                );
                                if ui.small_button("Change").clicked() {
                                    self.start_region_selection(ctx);
                                }
                            });
                        } else {
                            if ui.button("Select Area").clicked() {
                                self.start_region_selection(ctx);
                            }
                        }
                        ui.add_space(10.0);
                    }

                    // Main RECORD Button
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
                        let target_region = if self.mode == SelectedMode::Region {
                            self.region
                        } else {
                            None
                        };

                        if let Err(err) = self.recorder.start(target_region) {
                            self.error_msg = Some(err);
                        }
                    }

                    ui.add_space(10.0);

                    // Subtitle / Specs
                    let spec_text = match (self.mode, self.region) {
                        (SelectedMode::Region, Some(r)) => format!("30 FPS  ·  MP4  ·  {}×{}", r.width, r.height),
                        _ => "30 FPS  ·  MP4  ·  Fullscreen".to_string(),
                    };

                    ui.label(
                        RichText::new(spec_text)
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(140, 145, 155)),
                    );
                }

                ui.add_space(10.0);

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
