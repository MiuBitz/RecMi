use crate::recorder::{spawn_global_f9_listener, Recorder, RecorderState, RecordingRegion};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2, ViewportCommand};
use std::sync::mpsc::Receiver;
use xcap::Monitor;

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
    selection_texture: Option<TextureHandle>,
    error_msg: Option<String>,
    was_recording: bool,
    f9_receiver: Receiver<()>,
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
            selection_texture: None,
            error_msg: None,
            was_recording: false,
            f9_receiver: spawn_global_f9_listener(),
        }
    }

    fn start_region_selection(&mut self, ctx: &egui::Context) {
        // Capture desktop snapshot for instant screen-freeze overlay
        if let Ok(monitors) = Monitor::all() {
            let primary = monitors
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .or_else(|| Monitor::all().ok()?.into_iter().next());

            if let Some(mon) = primary {
                if let Ok(img) = mon.capture_image() {
                    let size = [img.width() as usize, img.height() as usize];
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                    self.selection_texture = Some(ctx.load_texture(
                        "freeze_bg",
                        color_img,
                        TextureOptions::LINEAR,
                    ));
                }
            }
        }

        self.is_selecting_region = true;
        self.drag_start = None;
        self.drag_current = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
        ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn finish_region_selection(&mut self, ctx: &egui::Context) {
        if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
            let ppp = ctx.pixels_per_point();

            let min_x = (start.x.min(current.x).max(0.0) * ppp).round() as u32;
            let min_y = (start.y.min(current.y).max(0.0) * ppp).round() as u32;
            let max_x = (start.x.max(current.x).max(0.0) * ppp).round() as u32;
            let max_y = (start.y.max(current.y).max(0.0) * ppp).round() as u32;

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
        self.selection_texture = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::Decorations(true));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(340.0, 240.0)));
    }

    fn cancel_region_selection(&mut self, ctx: &egui::Context) {
        self.is_selecting_region = false;
        self.drag_start = None;
        self.drag_current = None;
        self.selection_texture = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::Decorations(true));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(340.0, 240.0)));
    }

    fn toggle_recording(&mut self) {
        if self.recorder.is_recording() {
            self.recorder.stop();
        } else {
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
    }
}

impl eframe::App for RecMiApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 1. RECEIVE WIN32 SYSTEM-WIDE F9 HOTKEY MESSAGES ---
        while let Ok(()) = self.f9_receiver.try_recv() {
            self.toggle_recording();
        }

        // --- 2. SCREEN-FREEZE REGION SELECTION OVERLAY ---
        if self.is_selecting_region {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel_region_selection(ctx);
                return;
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::BLACK))
                .show(ctx, |ui| {
                    let screen_rect = ui.available_rect_before_wrap();
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

                    // Render Captured Desktop Background Image
                    if let Some(ref texture) = self.selection_texture {
                        painter.image(
                            texture.id(),
                            screen_rect,
                            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    let dark_tint = Color32::from_black_alpha(90);

                    // Render Cutout & Dimmed Overlay
                    if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
                        let sel_rect = Rect::from_two_pos(start, current);

                        // Mask 4 outer rectangles around selection box
                        let top_rect = Rect::from_min_max(screen_rect.min, Pos2::new(screen_rect.max.x, sel_rect.min.y));
                        painter.rect_filled(top_rect, 0.0, dark_tint);

                        let bot_rect = Rect::from_min_max(Pos2::new(screen_rect.min.x, sel_rect.max.y), screen_rect.max);
                        painter.rect_filled(bot_rect, 0.0, dark_tint);

                        let left_rect = Rect::from_min_max(
                            Pos2::new(screen_rect.min.x, sel_rect.min.y),
                            Pos2::new(sel_rect.min.x, sel_rect.max.y),
                        );
                        painter.rect_filled(left_rect, 0.0, dark_tint);

                        let right_rect = Rect::from_min_max(
                            Pos2::new(sel_rect.max.x, sel_rect.min.y),
                            Pos2::new(screen_rect.max.x, sel_rect.max.y),
                        );
                        painter.rect_filled(right_rect, 0.0, dark_tint);

                        // Red Selection Border Line
                        painter.rect_stroke(
                            sel_rect,
                            0.0,
                            Stroke::new(2.5f32, Color32::from_rgb(229, 9, 20)),
                        );

                        // Dimensions Label Badge
                        let ppp = ctx.pixels_per_point();
                        let w = ((sel_rect.width().abs() * ppp).round() as u32) & !1;
                        let h = ((sel_rect.height().abs() * ppp).round() as u32) & !1;
                        let badge_text = format!(" {} × {} px ", w, h);

                        let badge_pos = Pos2::new(
                            sel_rect.left().max(10.0),
                            (sel_rect.top() - 28.0).max(10.0),
                        );

                        painter.text(
                            badge_pos,
                            Align2::LEFT_TOP,
                            badge_text,
                            FontId::monospace(14.0),
                            Color32::WHITE,
                        );
                    } else {
                        // Dim screen before drag starts
                        painter.rect_filled(screen_rect, 0.0, dark_tint);
                        painter.text(
                            screen_rect.center(),
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

        // --- 3. MAIN APPLICATION & RECORDING LIFECYCLE ---
        let is_recording = self.recorder.is_recording();

        if is_recording && !self.was_recording {
            // Auto-minimize recorder window when recording starts in ANY mode
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        } else if !is_recording && self.was_recording {
            // Restore window when recording stops
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(340.0, 240.0)));
        }
        self.was_recording = is_recording;

        if is_recording {
            ctx.request_repaint();
        }

        // Standard App Panel
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
                    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

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
                        RichText::new("STOP (F9)")
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

                    let record_btn = egui::Button::new(
                        RichText::new("● RECORD (F9)")
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(229, 9, 20))
                    .min_size(Vec2::new(180.0, 44.0));

                    if ui.add(record_btn).clicked() {
                        self.toggle_recording();
                    }

                    ui.add_space(10.0);

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
