# RecMi

Minimal Windows screen recorder built with Rust and Tauri 2, with a plain HTML/CSS/JavaScript frontend.

## Build

Install Rust, Microsoft C++ Build Tools, and WebView2. Install FFmpeg with `gdigrab`, `libx264`, and AAC encoding. Put `ffmpeg.exe` on PATH or beside `recmi.exe`.

```powershell
cargo run
cargo test
node tests/geometry.cjs
cargo build --release
```

The executable is `target/release/recmi.exe`. The frontend is embedded; there is no Node build step. FFmpeg remains an external prerequisite.

## Recording

Choose Full screen (primary monitor) or Select area. Click Start recording or press F9. RecMi hides before capture starts. A small, draggable recording indicator stays on top and shows elapsed time. Click its stop button or press F9 to stop; the main window returns after MP4 finalization. The indicator uses Windows capture exclusion so the screen underneath it appears in the video. Recording is blocked if capture exclusion cannot be enabled. Recordings include the cursor and save to `%USERPROFILE%\Videos\RecMi`.

Sound defaults to **System sound**, capturing playback through your default Windows speakers or headphones using native WASAPI loopback. No Stereo Mix or virtual audio driver is needed. You can also choose **Microphone**, **System sound + microphone**, or **No sound**. Microphone capture uses your default Windows input device. Sound is stored as stereo AAC at 48 kHz. Audio devices are selected when recording starts; stop and restart after changing your default Windows device.

If Windows denies microphone access or a default device is unavailable, RecMi shows the error instead of silently recording without the requested audio. Recording starts once FFmpeg confirms a video frame. Closing RecMi normally finalizes an active recording.

## Select and edit an area

Select area opens a full-size snapshot overlay on the primary screen, with RecMi hidden from the snapshot.

- Drag outside the rectangle to draw a new area.
- Drag inside the rectangle to move it.
- Drag any of the eight corner/edge handles to resize it.
- Use arrow keys to nudge by one physical pixel; Shift + arrow moves by ten.
- Edit X, Y, Width and Height for precise dimensions.
- Drag the toolbar's title to move it out of the way.
- Choose Use area or press Enter to confirm; Escape cancels and keeps the previous area.
- Use Edit selected area in the main window to adjust the same rectangle later.

Coordinates map to physical screen pixels on scaled displays. Areas stay inside the primary monitor, with even video dimensions. A minimum 20 × 20 rectangle is enforced by the editor.

## Verification

Unit tests cover region bounds. `node tests/geometry.cjs` covers drawing in both directions, moving, eight resize handles, bounds and even dimensions. Live tests are opt-in and remove their own generated recordings:

```powershell
cargo test live_recording_is_playable -- --ignored --nocapture
cargo test system_audio_contains_sound -- --ignored --nocapture
cargo test indicator_exclusion_reveals_desktop_in_ffmpeg -- --ignored --nocapture
cargo test microphone_and_mixed_modes_finalize -- --ignored --nocapture
```

The system-audio test plays a quiet six-second tone and checks the AAC waveform for dropouts. The microphone test briefly captures the default input. The indicator test briefly shows overlapping test rectangles and verifies that FFmpeg captures the background underneath the excluded window. FFprobe is needed for video/audio metadata checks. Encoder errors retain a diagnostic `.log` beside the recording.

Windows is the supported platform. Pause/resume and video editing are not implemented.

The selector controller also has an interaction check: node tests/selector.cjs.

