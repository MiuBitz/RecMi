# RecMi — Project Plan

## 1. Project Overview

**RecMi** is a very small, fast, distraction-free desktop screen recorder.

The goal is not to compete with OBS or other full-featured recording software. The goal is:

> **Open. Record. Done.**

The first release should focus on reliable screen recording with almost no interface or configuration.

---

## 2. MVP Goal

Build a working desktop recorder that can:

- Record the primary/full monitor
- Record at 30 FPS
- Encode to MP4
- Start and stop recording
- Automatically save recordings
- Use simple, minimal UI
- Run with low overhead

The MVP should be intentionally small.

---

## 3. MVP Features

### Recording

- [x] Full monitor recording
- [x] 30 FPS
- [x] MP4 output
- [x] Start recording
- [x] Stop recording
- [x] Automatic file naming
- [x] Save recordings to a predictable location

### User Interface

Initial UI:

```text
┌────────────────────────────┐
│                            │
│       RecMi          │
│                            │
│        ● RECORD            │
│                            │
│        30 FPS · MP4        │
│                            │
└────────────────────────────┘
```

While recording:

```text
┌────────────────────────────┐
│                            │
│        ● 00:13             │
│                            │
│          STOP              │
│                            │
└────────────────────────────┘
```

The UI should remain extremely simple.

### File Naming

Recordings should automatically receive names such as:

```text
2026-08-17_19-52-31.mp4
```

No manual filename is required for the MVP.

---

## 4. Explicitly Out of Scope for MVP

Do **not** add these features during the first version:

- Microphone recording
- System audio
- Region recording
- Window recording
- Webcam
- Pause/resume
- Video editing
- GIF export
- Annotations
- Cursor effects
- Click visualization
- Streaming
- Cloud uploads
- Accounts
- Online features
- Complicated settings
- Themes
- Plugin system

These can be considered later.

---

# 5. Technology Stack

## Core

- **Rust** — application language
- **egui / eframe** — minimal desktop UI
- **xcap** — screen capture
- **FFmpeg** — video encoding
- **chrono** — automatic recording filenames

## Initial Architecture

```text
┌──────────────────────┐
│      RecMi     │
│                      │
│      Rust + egui     │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│     Screen Capture   │
│        xcap          │
└──────────┬───────────┘
           │
       video frames
           │
           ▼
┌──────────────────────┐
│       FFmpeg         │
│      H.264 / MP4     │
└──────────┬───────────┘
           │
           ▼
      recording.mp4
```

For the first implementation, FFmpeg can run as a separate executable instead of being compiled directly into the application.

---

# 6. Development Milestones

## M1 — Rust Project

Create the project and confirm that Rust builds successfully.

Expected structure:

```text
RecMi/
├── Cargo.toml
└── src/
    └── main.rs
```

Success condition:

```text
cargo run
```

works.

---

## M2 — Screen Capture

Use xcap to detect the available monitors and capture a screenshot.

Success condition:

```text
Found 1 monitor(s)
Monitor 0: 1920x1080
Screenshot saved!
```

A PNG screenshot should be generated successfully.

---

## M3 — Continuous Capture

Capture frames continuously.

Target:

```text
30 frames / second
```

The first version should prioritize reliability over perfect timing.

Success condition:

- Capture runs continuously
- CPU usage is reasonable
- Frames are not corrupted

---

## M4 — FFmpeg Pipeline

Start FFmpeg from Rust and pipe captured frames into it.

Pipeline:

```text
xcap
  ↓
raw frames
  ↓
Rust
  ↓
FFmpeg stdin
  ↓
H.264
  ↓
MP4
```

Success condition:

A playable MP4 file is produced.

---

## M5 — Recording Lifecycle

Implement:

```text
START
  ↓
CAPTURE
  ↓
ENCODE
  ↓
STOP
  ↓
FINALIZE MP4
  ↓
SAVE
```

Important requirements:

- FFmpeg must exit cleanly
- MP4 must be finalized correctly
- No corrupted recordings after normal stop
- Recording process must not remain running

---

## M6 — Minimal UI

Add the first interface.

Idle:

```text
┌────────────────────────────┐
│                            │
│       RecMi          │
│                            │
│        ● RECORD            │
│                            │
└────────────────────────────┘
```

Recording:

```text
┌────────────────────────────┐
│                            │
│        ● 00:13             │
│                            │
│          STOP              │
│                            │
└────────────────────────────┘
```

The UI should not contain unnecessary controls.

---

## M7 — Hide Recorder UI

The recorder application itself should not appear in the recording.

When recording starts:

```text
UI visible
    ↓
Start recording
    ↓
Hide/minimize recorder
    ↓
Capture screen
```

When recording stops:

```text
Stop
  ↓
Show recorder
  ↓
Display saved state
```

---

## M8 — Windows Build

Create a standalone Windows release.

Target:

```text
RecMi.exe
```

Test on a clean Windows environment.

Check:

- Application starts
- Monitor detection works
- Recording works
- MP4 plays correctly
- Stop works correctly
- File saving works

---

# 7. Project Structure

Start simple.

```text
RecMi/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs
│   ├── recorder.rs
│   └── ui.rs
└── assets/
```

Additional modules should only be introduced when they become necessary.

---

# 8. Future Features

These are possible after the MVP is stable.

## Recording Options

- [ ] 60 FPS
- [ ] Region recording
- [ ] Window recording
- [ ] Monitor selection
- [ ] Resolution selection
- [ ] Quality selector
- [ ] Bitrate control

## Audio

- [ ] Microphone
- [ ] System audio
- [ ] Microphone + system audio
- [ ] Audio device selection

## Convenience

- [ ] Global start/stop hotkey
- [ ] Pause/resume
- [ ] Countdown
- [ ] Open recordings folder
- [ ] Recent recordings
- [ ] Custom save location

## Visual Features

- [ ] Cursor capture
- [ ] Cursor highlighting
- [ ] Click visualization
- [ ] Recording indicator
- [ ] Screenshot capture

## Export

- [ ] WebM
- [ ] GIF
- [ ] Custom codecs
- [ ] Quality presets

---

# 9. Design Principles

RecMi should follow these rules.

### 1. Minimal

If a feature does not improve the basic recording workflow, it does not belong in the main UI.

### 2. Fast

The application should launch quickly and begin recording quickly.

### 3. Lightweight

Avoid large frameworks and unnecessary background services.

### 4. Local-first

Recordings should stay on the user's computer.

### 5. No Account

No login should ever be required to record a video.

### 6. No Cloud

The MVP should not depend on an internet connection.

### 7. Reliable

A simple recorder that produces a good MP4 every time is better than a feature-heavy recorder that sometimes fails.

---

# 10. MVP Success Criteria

RecMi 0.1 is complete when a user can:

1. Launch the application.
2. Click **Record**.
3. The recorder UI disappears.
4. The screen is captured at 30 FPS.
5. Click **Stop**.
6. The UI returns.
7. A correctly named MP4 appears automatically.
8. The MP4 plays correctly.

The entire experience should feel like:

```text
Open
 ↓
Record
 ↓
Do something
 ↓
Stop
 ↓
Done
```

---

# 11. Suggested Development Order

Build in this exact order:

```text
1. Rust project
2. Monitor detection
3. Screenshot capture
4. Continuous frame capture
5. FFmpeg integration
6. MP4 generation
7. Start/stop lifecycle
8. Minimal UI
9. Hide UI during recording
10. Automatic filenames
11. Windows release
```

Do not start adding secondary features until this pipeline is stable.

---

# 12. First Release

## RecMi 0.1

**Tagline:**

> Open. Record. Done.

### Included

- Full monitor recording
- 30 FPS
- MP4
- Start/Stop
- Automatic filenames
- Minimal UI
- Local-only operation
- Windows executable

### Not Included

Everything else.

The purpose of version 0.1 is to prove that the core experience is excellent before expanding the application.
