use xcap::Monitor;
use chrono::Local;
use image::{imageops, RgbaImage};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordingRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RecordingRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        let mut w = width & !1;
        let mut h = height & !1;
        if w < 2 { w = 2; }
        if h < 2 { h = 2; }
        Self { x, y, width: w, height: h }
    }
}

#[allow(dead_code)]
pub enum RecorderState {
    Idle,
    Recording {
        start_time: Instant,
        output_filename: String,
        region: Option<RecordingRegion>,
    },
    Finished {
        saved_path: String,
    },
    Error(String),
}

pub struct Recorder {
    state: RecorderState,
    stop_flag: Option<Arc<AtomicBool>>,
    threads: Option<(JoinHandle<()>, JoinHandle<()>)>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            state: RecorderState::Idle,
            stop_flag: None,
            threads: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.state, RecorderState::Recording { .. })
    }

    pub fn elapsed_secs(&self) -> u64 {
        match &self.state {
            RecorderState::Recording { start_time, .. } => start_time.elapsed().as_secs(),
            _ => 0,
        }
    }

    pub fn state(&self) -> &RecorderState {
        &self.state
    }

    pub fn start(&mut self, region: Option<RecordingRegion>) -> Result<(), String> {
        if self.is_recording() {
            return Ok(());
        }

        // 1. Detect target monitor
        let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {}", e))?;
        if monitors.is_empty() {
            return Err("No active monitors found".into());
        }

        let primary_mon = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok()?.into_iter().next())
            .ok_or("Primary monitor unavailable")?;

        let mon_w = primary_mon.width().map_err(|e| e.to_string())?;
        let mon_h = primary_mon.height().map_err(|e| e.to_string())?;

        // Determine final recording dimensions
        let (rec_w, rec_h, crop_info) = if let Some(r) = region {
            let clamped_x = r.x.min(mon_w.saturating_sub(2));
            let clamped_y = r.y.min(mon_h.saturating_sub(2));
            let clamped_w = (r.width & !1).clamp(2, mon_w - clamped_x);
            let clamped_h = (r.height & !1).clamp(2, mon_h - clamped_y);
            let final_region = RecordingRegion {
                x: clamped_x,
                y: clamped_y,
                width: clamped_w,
                height: clamped_h,
            };
            (clamped_w, clamped_h, Some(final_region))
        } else {
            (mon_w & !1, mon_h & !1, None)
        };

        // 2. Generate filename
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let output_filename = format!("{}.mp4", timestamp);

        // 3. Setup atomic flags & channel
        let stop_flag = Arc::new(AtomicBool::new(false));
        let is_running = Arc::clone(&stop_flag);

        let (tx, rx): (SyncSender<RgbaImage>, Receiver<RgbaImage>) = mpsc::sync_channel(60);

        // 4. Spawn Encoder Thread
        let enc_filename = output_filename.clone();
        let encoder_handle = thread::spawn(move || {
            let mut child = match Command::new("ffmpeg")
                .args([
                    "-f", "rawvideo",
                    "-pixel_format", "rgba",
                    "-video_size", &format!("{}x{}", rec_w, rec_h),
                    "-framerate", "30",
                    "-i", "pipe:0",
                    "-c:v", "libx264",
                    "-pix_fmt", "yuv420p",
                    "-preset", "ultrafast",
                    "-y",
                    &enc_filename,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to spawn FFmpeg: {}", e);
                    return;
                }
            };

            if let Some(mut stdin) = child.stdin.take() {
                while let Ok(frame) = rx.recv() {
                    if stdin.write_all(frame.as_raw()).is_err() {
                        break;
                    }
                }
            }

            let _ = child.wait();
        });

        // 5. Spawn Capture Thread
        let capture_handle = thread::spawn(move || {
            let monitors = match Monitor::all() {
                Ok(m) => m,
                Err(_) => return,
            };

            let monitor = match monitors.into_iter().find(|m| m.is_primary().unwrap_or(false)) {
                Some(m) => m,
                None => match Monitor::all() {
                    Ok(mut m) if !m.is_empty() => m.remove(0),
                    _ => return,
                },
            };

            let target_fps = 30.0;
            let frame_duration = Duration::from_secs_f64(1.0 / target_fps);

            while !is_running.load(Ordering::Relaxed) {
                let frame_start = Instant::now();

                if let Ok(full_image) = monitor.capture_image() {
                    let frame_to_send = if let Some(r) = crop_info {
                        imageops::crop_imm(&full_image, r.x, r.y, r.width, r.height).to_image()
                    } else {
                        full_image
                    };

                    let _ = tx.try_send(frame_to_send);
                }

                let elapsed = frame_start.elapsed();
                if elapsed < frame_duration {
                    thread::sleep(frame_duration - elapsed);
                }
            }
        });

        self.stop_flag = Some(stop_flag);
        self.threads = Some((capture_handle, encoder_handle));
        self.state = RecorderState::Recording {
            start_time: Instant::now(),
            output_filename,
            region: crop_info,
        };

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(flag) = self.stop_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }

        if let Some((cap_thread, enc_thread)) = self.threads.take() {
            let _ = cap_thread.join();
            let _ = enc_thread.join();
        }

        if let RecorderState::Recording { output_filename, .. } = &self.state {
            self.state = RecorderState::Finished {
                saved_path: output_filename.clone(),
            };
        }
    }
}

// Spawns native Win32 WM_HOTKEY thread to listen for F9 system-wide
pub fn spawn_global_f9_listener() -> Receiver<()> {
    let (tx, rx) = mpsc::channel();

    #[cfg(target_os = "windows")]
    thread::spawn(move || {
        #[link(name = "user32")]
        extern "system" {
            fn RegisterHotKey(hWnd: *mut std::ffi::c_void, id: i32, fsModifiers: u32, vk: u32) -> i32;
            fn GetMessageW(lpMsg: *mut Msg, hWnd: *mut std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
            fn UnregisterHotKey(hWnd: *mut std::ffi::c_void, id: i32) -> i32;
        }

        #[repr(C)]
        struct Point { x: i32, y: i32 }
        #[repr(C)]
        struct Msg {
            hwnd: *mut std::ffi::c_void,
            message: u32,
            wparam: usize,
            lparam: isize,
            time: u32,
            pt: Point,
        }

        const WM_HOTKEY: u32 = 0x0312;
        const MOD_NOREPEAT: u32 = 0x4000;
        const VK_F9: u32 = 0x78;

        unsafe {
            if RegisterHotKey(std::ptr::null_mut(), 1, MOD_NOREPEAT, VK_F9) != 0 {
                let mut msg: Msg = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_HOTKEY {
                        let _ = tx.send(());
                    }
                }
                UnregisterHotKey(std::ptr::null_mut(), 1);
            }
        }
    });

    rx
}
