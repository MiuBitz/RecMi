//! Native Windows audio capture. PCM is fed to FFmpeg; EOF finalizes the MP4.
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    process::ChildStdin,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AudioMode {
    #[default]
    System,
    Microphone,
    Both,
    Silent,
}
pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    input: Option<mpsc::Sender<ChildStdin>>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl AudioCapture {
    pub fn prepare(mode: AudioMode) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (input, rx) = mpsc::channel();
        let (ready, status) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = capture(mode, flag, rx, &ready);
            if let Err(e) = &result {
                let _ = ready.send(Err(e.clone()));
            }
            result
        });
        let mut capture = Self {
            stop,
            input: Some(input),
            worker: Some(worker),
        };
        match status.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(capture),
            result => {
                capture.request_stop();
                Err(match result {
                    Ok(Err(e)) => format!("Audio unavailable: {e}"),
                    _ => "Audio device initialization timed out".into(),
                })
            }
        }
    }
    pub fn attach(&mut self, input: ChildStdin) -> Result<(), String> {
        self.input
            .take()
            .ok_or("Audio already attached")?
            .send(input)
            .map_err(|_| "Audio capture stopped before recording began".into())
    }
    pub fn request_stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.input.take();
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.request_stop();
        self.worker
            .take()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("Audio capture thread failed".into()))
            })
            .unwrap_or(Ok(()))
    }
}
impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.request_stop(); /* Child process shutdown unblocks any pending pipe write. */
    }
}
#[cfg(windows)]
fn capture(
    mode: AudioMode,
    stop: Arc<AtomicBool>,
    input: mpsc::Receiver<ChildStdin>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    fn run(
        mode: AudioMode,
        stop: Arc<AtomicBool>,
        input: mpsc::Receiver<ChildStdin>,
        ready: &mpsc::Sender<Result<(), String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::VecDeque;
        use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
        wasapi::initialize_mta().ok()?;
        struct Com;
        impl Drop for Com {
            fn drop(&mut self) {
                wasapi::deinitialize();
            }
        }
        let _com = Com;
        let enumerator = DeviceEnumerator::new()?;
        let format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);
        let directions = match mode {
            AudioMode::Both => vec![Direction::Render, Direction::Capture],
            AudioMode::Microphone => vec![Direction::Capture],
            _ => vec![Direction::Render],
        };
        let mut clients = Vec::new();
        for direction in directions {
            let device = enumerator.get_default_device(&direction)?;
            let mut client = device.get_iaudioclient()?;
            client.initialize_client(
                &format,
                &Direction::Capture,
                &StreamMode::PollingShared {
                    autoconvert: true,
                    buffer_duration_hns: 10_000_000,
                },
            )?;
            let reader = client.get_audiocaptureclient()?;
            clients.push((client, reader, VecDeque::<u8>::new()));
        }
        let _ = ready.send(Ok(()));
        let mut pipe = loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            match input.recv_timeout(Duration::from_millis(100)) {
                Ok(p) => break p,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                _ => {}
            }
        };
        for (client, _, _) in &clients {
            client.start_stream()?;
        }
        // Keep pipe backpressure off the device thread. A slow video encoder must
        // not stop us draining WASAPI's finite capture buffer.
        let (chunks, pending) = mpsc::sync_channel::<Vec<u8>>(500);
        let writer = thread::spawn(move || -> std::io::Result<()> {
            for chunk in pending {
                pipe.write_all(&chunk)?;
            }
            Ok(())
        });
        // Windows polling and device packets do not arrive at exact 10 ms
        // boundaries. Retain a small cushion instead of inserting gaps whenever
        // a packet arrives slightly late.
        let mut deadline = Instant::now() + Duration::from_millis(60);
        let mut chunk = vec![0u8; 480 * 2 * 4];
        let capture_result = (|| -> Result<(), Box<dyn std::error::Error>> {
            while !stop.load(Ordering::Acquire) {
                let now = Instant::now();
                if deadline > now {
                    thread::sleep(deadline - now);
                }
                for (_, reader, queue) in &mut clients {
                    while reader.get_next_packet_size()?.unwrap_or(0) > 0 {
                        let start = queue.len();
                        let info = reader.read_from_device_to_deque(queue)?;
                        // WASAPI marks silent packets explicitly; their payload is
                        // not valid sound and may contain old samples.
                        if info.flags.silent {
                            for byte in queue.iter_mut().skip(start) {
                                *byte = 0;
                            }
                        }
                    }
                }
                for sample in chunk.chunks_exact_mut(4) {
                    let mut value = 0.0f32;
                    for (_, _, queue) in &mut clients {
                        if queue.len() >= 4 {
                            let bytes = [
                                queue.pop_front().unwrap(),
                                queue.pop_front().unwrap(),
                                queue.pop_front().unwrap(),
                                queue.pop_front().unwrap(),
                            ];
                            value += f32::from_le_bytes(bytes);
                        }
                    }
                    // Leave headroom when combining playback and microphone.
                    let gain = 1.0 / clients.len() as f32;
                    sample.copy_from_slice(&(value * gain).clamp(-1.0, 1.0).to_le_bytes());
                }
                chunks
                    .try_send(chunk.clone())
                    .map_err(|e| format!("Audio encoder cannot keep up: {e}"))?;
                deadline += Duration::from_millis(10);
            }
            Ok(())
        })();
        for (client, _, _) in &clients {
            let _ = client.stop_stream();
        }
        drop(chunks);
        let write_result = writer.join().map_err(|_| "Audio writer thread failed")?;
        capture_result?;
        write_result?;
        Ok(())
    }
    run(mode, stop, input, ready).map_err(|e| e.to_string())
}
#[cfg(not(windows))]
fn capture(
    _: AudioMode,
    _: Arc<AtomicBool>,
    _: mpsc::Receiver<ChildStdin>,
    _: &mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    Err("Native audio capture requires Windows".into())
}
