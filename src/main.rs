use xcap::Monitor;
use chrono::Local;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RecMi Continuous Frame Capture & FFmpeg Encoding (Step 2) ===");

    // 1. Enumerate monitors
    let monitors = Monitor::all()?;
    if monitors.is_empty() {
        return Err("No active monitors detected!".into());
    }

    // Pick primary monitor or first available
    let primary_mon = monitors.iter().find(|m| m.is_primary().unwrap_or(false));
    let monitor = primary_mon.unwrap_or(&monitors[0]);

    let width = monitor.width()?;
    let height = monitor.height()?;
    let name = monitor.name()?;

    println!("Target Monitor: \"{}\" ({}x{})", name, width, height);

    // 2. Generate timestamped output filename
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let output_filename = format!("{}.mp4", timestamp);
    println!("Output File: {}", output_filename);

    // 3. Spawn FFmpeg process
    let mut child = Command::new("ffmpeg")
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
            &output_filename,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn FFmpeg process: {}. Is FFmpeg installed in PATH?", e))?;

    let mut stdin = child.stdin.take().ok_or("Failed to open FFmpeg stdin pipe")?;

    // 4. Continuous recording loop at 30 FPS for 5 seconds
    let target_fps = 30.0;
    let frame_duration = Duration::from_secs_f64(1.0 / target_fps);
    let total_test_duration = Duration::from_secs(5);
    let expected_frames = (target_fps * total_test_duration.as_secs_f64()) as usize;

    println!("\nStarting 5-second test recording (~{} frames at 30 FPS)...", expected_frames);

    let recording_start = Instant::now();
    let mut frames_captured = 0;
    let mut error_count = 0;

    while recording_start.elapsed() < total_test_duration {
        let frame_start = Instant::now();

        match monitor.capture_image() {
            Ok(image) => {
                if let Err(e) = stdin.write_all(image.as_raw()) {
                    eprintln!("Failed to write frame to FFmpeg pipe: {}", e);
                    break;
                }
                frames_captured += 1;
            }
            Err(e) => {
                error_count += 1;
                if error_count <= 5 {
                    eprintln!("Frame capture error: {}", e);
                }
            }
        }

        // Frame rate pacing
        let elapsed_frame = frame_start.elapsed();
        if elapsed_frame < frame_duration {
            std::thread::sleep(frame_duration - elapsed_frame);
        }

        let elapsed_secs = recording_start.elapsed().as_secs();
        if frames_captured > 0 && frames_captured % 30 == 0 {
            println!("  [{}/5s] Captured {} frames...", elapsed_secs, frames_captured);
        }
    }

    let actual_duration = recording_start.elapsed();
    println!("\nRecording loop complete. Closing FFmpeg pipe...");

    // 5. Signal EOF to FFmpeg and wait for container finalization
    drop(stdin);

    let status = child.wait()?;
    let actual_fps = frames_captured as f64 / actual_duration.as_secs_f64();

    println!("--------------------------------------------------");
    if status.success() {
        println!("SUCCESS: Video encoded successfully!");
        println!(
            "Captured {} frames in {:.2?} (Average: {:.1} FPS, Errors: {})",
            frames_captured, actual_duration, actual_fps, error_count
        );

        if Path::new(&output_filename).exists() {
            let file_size = std::fs::metadata(&output_filename)?.len();
            let size_mb = file_size as f64 / (1024.0 * 1024.0);
            println!("Output file saved: {} ({:.2} MB)", output_filename, size_mb);
        }
    } else {
        eprintln!("FFmpeg exited with non-zero exit status: {:?}", status);
    }
    println!("--------------------------------------------------");

    Ok(())
}
