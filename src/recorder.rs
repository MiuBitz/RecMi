use xcap::Monitor;
use chrono::Local;
use image::RgbaImage;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub enum RecorderState {
    Idle,
    Recording {
        start_time: Instant,
        output_filename: String,
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

    pub fn start(&mut self) -> Result<(), String> {
        if self.is_recording() {
            return Ok(());
        }

        // 1. Detect target monitor resolution
        let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {}", e))?;
        if monitors.is_empty() {
            return Err("No active monitors found".into());
        }

        let primary_mon = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok()?.into_iter().next())
            .ok_or("Primary monitor unavailable")?;

        let width = primary_mon.width().map_err(|e| e.to_string())?;
        let height = primary_mon.height().map_err(|e| e.to_string())?;

        // 2. Generate filename
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let output_filename = format!("{}.mp4", timestamp);

        // 3. Setup atomic flags & channel
        let stop_flag = Arc::new(AtomicBool::new(false));
        let is_running = Arc::clone(&stop_flag);

        // Bounded channel (max 60 frames buffered in RAM)
        let (tx, rx): (SyncSender<RgbaImage>, Receiver<RgbaImage>) = mpsc::sync_channel(60);

        // 4. Spawn Encoder Thread
        let enc_filename = output_filename.clone();
        let encoder_handle = thread::spawn(move || {
            let mut child = match Command::new("ffmpeg")
                .args([
                    "-f", "rawvideo",
                    "-pixel_format", "rgba",
                    "-video_size", &format!("{}x{}", width, height),
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
                // Pipe auto-closes when stdin drops
            }

            let _ = child.wait();
        });

        // 5. Spawn Capture Thread (re-queries Monitor inside thread for thread safety)
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

                if let Ok(image) = monitor.capture_image() {
                    // Send to encoder channel (drop frame if encoder lags)
                    let _ = tx.try_send(image);
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
