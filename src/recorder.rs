use crate::audio::{AudioCapture, AudioMode};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use xcap::Monitor;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Region {
    pub fn checked(self, w: u32, h: u32) -> Result<Self, String> {
        if self.x >= w || self.y >= h {
            return Err("Area starts outside the monitor".into());
        }
        let width = self.width.min(w - self.x) & !1;
        let height = self.height.min(h - self.y) & !1;
        if width < 2 || height < 2 {
            return Err("Select an area at least 2 × 2 pixels".into());
        }
        Ok(Self {
            width,
            height,
            ..self
        })
    }
}
pub fn primary() -> Result<Monitor, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .cloned()
        .or_else(|| monitors.into_iter().next())
        .ok_or("No monitor available".into())
}
pub fn folder() -> Result<PathBuf, String> {
    let home = std::env::var_os("USERPROFILE").ok_or("USERPROFILE is unavailable")?;
    let path = PathBuf::from(home).join("Videos").join("RecMi");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn encoder() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("ffmpeg.exe")));
    beside
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}
fn command() -> Command {
    let mut cmd = Command::new(encoder());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
#[derive(Serialize, Clone)]
pub struct Status {
    pub recording: bool,
    pub elapsed: u64,
    pub saved: Option<String>,
    pub error: Option<String>,
}
struct Session {
    child: Child,
    audio: Option<AudioCapture>,
    progress: std::thread::JoinHandle<()>,
    started: Instant,
    path: PathBuf,
    log: PathBuf,
}
#[derive(Default)]
pub struct Recorder {
    session: Option<Session>,
    saved: Option<String>,
    error: Option<String>,
}
impl Recorder {
    pub fn report_error(&mut self, error: String) {
        self.error = Some(error);
    }
    pub fn saved_path(&self) -> Option<&str> {
        self.saved.as_deref()
    }
    pub fn active(&self) -> bool {
        self.session.is_some()
    }
    pub fn start(&mut self, region: Option<Region>, mode: AudioMode) -> Result<(), String> {
        if self.active() {
            return Err("Already recording".into());
        }
        self.error = None;
        let monitor = primary()?;
        let w = monitor.width().map_err(|e| e.to_string())?;
        let h = monitor.height().map_err(|e| e.to_string())?;
        let r = region
            .unwrap_or(Region {
                x: 0,
                y: 0,
                width: w,
                height: h,
            })
            .checked(w, h)?;
        let path = folder()?.join(format!(
            "{}.mp4",
            Local::now().format("%Y-%m-%d_%H-%M-%S_%f")
        ));
        let log = path.with_extension("log");
        let stderr = File::create(&log).map_err(|e| e.to_string())?;
        let mut audio = if mode == AudioMode::Silent {
            None
        } else {
            Some(AudioCapture::prepare(mode)?)
        };
        let mut cmd = command();
        cmd.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-progress",
            "pipe:1",
            "-stats_period",
            "0.1",
            "-n",
            "-thread_queue_size",
            "512",
            "-probesize",
            "32",
            "-use_wallclock_as_timestamps",
            "1",
            "-f",
            "gdigrab",
            "-framerate",
            "30",
            "-draw_mouse",
            "1",
            "-offset_x",
            &(monitor.x().map_err(|e| e.to_string())? + r.x as i32).to_string(),
            "-offset_y",
            &(monitor.y().map_err(|e| e.to_string())? + r.y as i32).to_string(),
            "-video_size",
            &format!("{}x{}", r.width, r.height),
            "-i",
            "desktop",
        ]);
        if audio.is_some() {
            cmd.args([
                "-thread_queue_size",
                "512",
                "-probesize",
                "32",
                "-analyzeduration",
                "0",
                "-f",
                "f32le",
                "-ar",
                "48000",
                "-ac",
                "2",
                "-i",
                "pipe:0",
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-af",
                "aresample=async=1:first_pts=0",
                "-shortest",
                "-shortest_buf_duration",
                "0.5",
            ]);
        } else {
            cmd.arg("-an");
        }
        let mut child = cmd.args(["-c:v","libx264","-preset","ultrafast","-crf","23","-pix_fmt","yuv420p","-movflags","+faststart"])
            .arg(&path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(stderr).spawn()
            .map_err(|e| format!("Cannot start FFmpeg: {e}. Install FFmpeg or place ffmpeg.exe beside RecMi.exe."))?;
        if let Some(capture) = &mut audio {
            if let Err(e) = capture.attach(child.stdin.take().expect("Audio input pipe")) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        }
        let output = child.stdout.take().expect("FFmpeg progress pipe");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let progress = std::thread::spawn(move || {
            let mut announced = false;
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else {
                    break;
                };
                if !announced
                    && line
                        .strip_prefix("frame=")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .unwrap_or(0)
                        > 0
                {
                    let _ = ready_tx.send(());
                    announced = true;
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if ready_rx.recv_timeout(Duration::from_millis(100)).is_ok() {
                break;
            }
            if !matches!(child.try_wait(), Ok(None)) || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = progress.join();
                if let Some(capture) = &mut audio {
                    let _ = capture.finish();
                }
                return Err(format!(
                    "FFmpeg did not start capturing: {}",
                    fs::read_to_string(&log).unwrap_or_default()
                ));
            }
        }
        self.session = Some(Session {
            child,
            audio,
            progress,
            started: Instant::now(),
            path,
            log,
        });
        Ok(())
    }
    fn finish(&mut self, mut s: Session, request_stop: bool) -> Result<(), String> {
        if request_stop {
            if let Some(audio) = &mut s.audio {
                audio.request_stop();
            }
            if let Some(mut input) = s.child.stdin.take() {
                let _ = input.write_all(b"q\n");
            }
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let exit = loop {
            match s.child.try_wait() {
                Ok(Some(exit)) => break exit,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                _ => {
                    let _ = s.child.kill();
                    let _ = s.child.wait();
                    let _ = s.progress.join();
                    if let Some(audio) = &mut s.audio {
                        let _ = audio.finish();
                    }
                    return Err("Encoder did not finalize within 20 seconds; the recording may be incomplete.".into());
                }
            }
        };
        let _ = s.progress.join();
        let audio_result = s.audio.as_mut().map(|a| a.finish()).unwrap_or(Ok(()));
        if !exit.success() || fs::metadata(&s.path).map(|m| m.len() == 0).unwrap_or(true) {
            return Err(format!(
                "Recording failed: {}",
                fs::read_to_string(&s.log).unwrap_or_else(|_| exit.to_string())
            ));
        }
        self.saved = Some(s.path.to_string_lossy().into_owned());
        if let Err(e) = audio_result {
            return Err(format!("Video saved, but audio capture failed: {e}"));
        }
        let _ = fs::remove_file(s.log);
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(s) = self.session.take() {
            if let Err(e) = self.finish(s, true) {
                self.error = Some(e.clone());
                return Err(e);
            }
        }
        Ok(())
    }
    pub fn status(&mut self) -> Status {
        let exited = self
            .session
            .as_mut()
            .map(|s| !matches!(s.child.try_wait(), Ok(None)))
            .unwrap_or(false);
        if exited {
            let s = self.session.take().unwrap();
            self.error = Some(match self.finish(s, false) {
                Err(e) => e,
                Ok(()) => "Encoder stopped unexpectedly; the completed file was saved.".into(),
            });
        }
        Status {
            recording: self.active(),
            elapsed: self
                .session
                .as_ref()
                .map(|s| s.started.elapsed().as_secs())
                .unwrap_or(0),
            saved: self.saved.clone(),
            error: self.error.clone(),
        }
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_crop_stays_even() {
        let r = Region {
            x: 99,
            y: 0,
            width: 10,
            height: 9,
        };
        assert!(r.checked(100, 100).is_err());
        let r = Region { x: 97, ..r }.checked(100, 100).unwrap();
        assert_eq!((r.width, r.height), (2, 8));
    }
    #[test]
    fn rejects_outside_and_zero() {
        assert!(Region {
            x: 100,
            y: 0,
            width: 20,
            height: 20
        }
        .checked(100, 100)
        .is_err());
        assert!(Region {
            x: 0,
            y: 0,
            width: 0,
            height: 10
        }
        .checked(100, 100)
        .is_err());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    #[ignore = "captures the live Windows desktop and requires FFmpeg"]
    fn live_recording_is_playable() {
        let mut recorder = Recorder::default();
        recorder
            .start(
                Some(Region {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                }),
                AudioMode::Silent,
            )
            .unwrap();
        std::thread::sleep(Duration::from_secs(2));
        recorder.stop().unwrap();
        let status = recorder.status();
        assert!(!status.recording);
        assert!(status.error.is_none());
        let path = status.saved.unwrap();
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,codec_name,r_frame_rate:format=duration",
                "-of",
                "json",
                &path,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let details = String::from_utf8_lossy(&output.stdout);
        assert!(details.contains("h264"));
        assert!(details.contains("320"));
        assert!(details.contains("240"));
        assert!(details.contains("30/1"));
        let duration = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=nw=1:nk=1",
                &path,
            ])
            .output()
            .unwrap();
        let seconds: f64 = String::from_utf8_lossy(&duration.stdout)
            .trim()
            .parse()
            .unwrap();
        assert!(
            (1.5..5.0).contains(&seconds),
            "Unexpected duration: {seconds}"
        );
        let decoded = command()
            .args(["-v", "error", "-i", &path, "-f", "null", "-"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        println!("Verified recording: {path}\n{details}");
        fs::remove_file(path).unwrap();

        // Normal application teardown must also finalize fullscreen MP4.
        recorder.start(None, AudioMode::Silent).unwrap();
        let path = recorder.session.as_ref().unwrap().path.clone();
        std::thread::sleep(Duration::from_secs(2));
        drop(recorder);
        let decoded = command()
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-f", "null", "-"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert!(fs::metadata(&path).unwrap().len() > 0);
        println!("Verified fullscreen recording finalized on drop");
        fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    #[test]
    #[ignore = "plays a quiet test tone and captures the Windows default output"]
    fn system_audio_contains_sound() {
        let tone =
            std::env::temp_dir().join(format!("recmi-tone-{}.wav", Local::now().format("%s_%f")));
        let generated = command()
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=6",
                "-af",
                "volume=0.2",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&tone)
            .output()
            .unwrap();
        assert!(generated.status.success());
        let mut recorder = Recorder::default();
        recorder
            .start(
                Some(Region {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                }),
                AudioMode::System,
            )
            .unwrap();
        let playback = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(New-Object System.Media.SoundPlayer $env:RECMI_TEST_TONE).PlaySync()",
            ])
            .env("RECMI_TEST_TONE", &tone)
            .status()
            .unwrap();
        assert!(playback.success());
        std::thread::sleep(Duration::from_millis(400));
        recorder.stop().unwrap();
        let path = recorder.status().saved.unwrap();
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=codec_name,sample_rate,channels",
                "-of",
                "json",
                &path,
            ])
            .output()
            .unwrap();
        let metadata = String::from_utf8_lossy(&probe.stdout);
        assert!(metadata.contains("aac"), "Missing AAC track: {metadata}");
        let decoded = command()
            .args([
                "-v", "error", "-i", &path, "-map", "0:a:0", "-f", "f32le", "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        let samples: Vec<f32> = decoded
            .stdout
            .chunks_exact(4)
            .map(|s| f32::from_le_bytes(s.try_into().unwrap()))
            .collect();
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.001, "Loopback audio is silent (peak {peak})");
        // A nonzero peak alone misses the original choppy-audio failure.
        // Check every 20 ms window within the sustained tone for dropouts.
        let levels: Vec<f32> = samples
            .chunks_exact(960 * 2)
            .map(|window| (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt())
            .collect();
        let threshold = 0.002;
        let first = levels
            .iter()
            .position(|v| *v > threshold)
            .expect("Missing tone");
        let last = levels.iter().rposition(|v| *v > threshold).unwrap();
        assert!(
            (last - first) >= 280,
            "Tone was truncated or time-compressed"
        );
        let dropouts = levels[first + 2..last - 2]
            .iter()
            .filter(|v| **v < threshold)
            .count();
        assert_eq!(dropouts, 0, "Sustained tone contains silent gaps");
        println!("Verified continuous system capture: peak={peak}; {metadata}");
        fs::remove_file(path).unwrap();
        fs::remove_file(tone).unwrap();
    }
}

#[cfg(test)]
mod microphone_tests {
    use super::*;
    #[test]
    #[ignore = "briefly captures the default microphone, then deletes its test recordings"]
    fn microphone_and_mixed_modes_finalize() {
        for (mode, label) in [
            (AudioMode::Microphone, "microphone"),
            (AudioMode::Both, "mixed"),
        ] {
            let mut recorder = Recorder::default();
            recorder
                .start(
                    Some(Region {
                        x: 0,
                        y: 0,
                        width: 320,
                        height: 240,
                    }),
                    mode,
                )
                .unwrap();
            std::thread::sleep(Duration::from_secs(1));
            recorder.stop().unwrap();
            let path = recorder.status().saved.unwrap();
            let decoded = command()
                .args([
                    "-v", "error", "-i", &path, "-map", "0:a:0", "-f", "null", "-",
                ])
                .output()
                .unwrap();
            assert!(
                decoded.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&decoded.stderr)
            );
            println!("Verified {label} audio track and normal finalization");
            fs::remove_file(path).unwrap();
        }
    }
}
