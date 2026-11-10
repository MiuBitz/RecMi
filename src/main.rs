#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod indicator;
mod recorder;
use audio::AudioMode;
use base64::Engine;
use recorder::{Recorder, Region, Status};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
#[derive(Default)]
struct AppState {
    recorder: Recorder,
    region: Option<Region>,
    selecting: bool,
    shortcut_error: Option<String>,
    audio: AudioMode,
    selection: Option<Snapshot>,
}
type Shared = Mutex<AppState>;
fn toggle(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Shared>();
    let mut state = state.lock().map_err(|e| e.to_string())?;
    if state.selecting {
        return Err("Finish selecting the area first".into());
    }
    let window = app.get_webview_window("main").ok_or("Window unavailable")?;
    if state.recorder.active() {
        let result = state.recorder.stop();
        indicator::restore(app);
        result
    } else {
        let indicator = indicator::prepare(app)?;
        window.hide().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(250));
        let region = state.region;
        let audio = state.audio;
        let result = state.recorder.start(region, audio).and_then(|()| {
            indicator.show().map_err(|e| {
                let _ = state.recorder.stop();
                format!("Cannot show recording indicator: {e}")
            })
        });
        if result.is_err() {
            indicator::restore(app);
        }
        if let Err(error) = &result {
            state.recorder.report_error(error.clone());
        }
        result
    }
}
#[tauri::command]
async fn toggle_recording(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || toggle(&app))
        .await
        .map_err(|e| e.to_string())?
}
// Stopping is idempotent: the widget and F9 may be pressed together.
#[tauri::command]
async fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = app.state::<Shared>();
        let mut state = shared.lock().map_err(|e| e.to_string())?;
        let result = state.recorder.stop();
        indicator::restore(&app);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn status(app: tauri::AppHandle) -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        let was = state.recorder.active();
        let mut status = state.recorder.status();
        if status.error.is_none() {
            status.error = state.shortcut_error.clone();
        }
        if was && !status.recording {
            indicator::restore(&app);
        }
        Ok(status)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn configure(
    region: Option<Region>,
    audio: AudioMode,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if state.recorder.active() || state.selecting {
            return Err("Finish recording or area selection before changing options".into());
        }
        state.region = region;
        state.audio = audio;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(serde::Serialize, Clone)]
struct Snapshot {
    image: String,
    width: u32,
    height: u32,
}
fn snapshot() -> Result<Snapshot, String> {
    let image = recorder::primary()?
        .capture_image()
        .map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();
    let mut buffer = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(Snapshot {
        image: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buffer.into_inner())
        ),
        width,
        height,
    })
}
#[derive(serde::Serialize)]
struct SelectionData {
    snapshot: Snapshot,
    region: Option<Region>,
}
#[tauri::command]
fn selection_data(state: tauri::State<Shared>) -> Result<SelectionData, String> {
    let state = state.try_lock().map_err(|_| "Recorder is busy")?;
    Ok(SelectionData {
        snapshot: state.selection.clone().ok_or("No selection active")?,
        region: state.region,
    })
}
fn finish_selection_impl(
    app: &tauri::AppHandle,
    region: Option<Region>,
    commit: bool,
) -> Result<(), String> {
    let state = app.state::<Shared>();
    let selected = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if commit {
            let shot = state.selection.as_ref().ok_or("No selection active")?;
            let region = region
                .ok_or("Draw an area first")?
                .checked(shot.width, shot.height)?;
            state.region = Some(region);
        }
        state.selecting = false;
        state.selection = None;
        state.region
    };
    if let Some(window) = app.get_webview_window("selector") {
        let _ = window.destroy();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("selection-changed", selected);
    Ok(())
}
#[tauri::command]
async fn finish_selection(
    app: tauri::AppHandle,
    region: Option<Region>,
    commit: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || finish_selection_impl(&app, region, commit))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn begin_selection(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        {
            let state = app.state::<Shared>();
            let mut state = state.lock().map_err(|e| e.to_string())?;
            if state.recorder.active() || state.selecting {
                return Err("Recorder is busy".into());
            }
            state.selecting = true;
        }
        let result = (|| -> Result<(), String> {
            let main = app.get_webview_window("main").ok_or("Window unavailable")?;
            main.hide().map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(250));
            let shot = snapshot()?;
            let monitor = recorder::primary()?;
            let position = tauri::PhysicalPosition::new(
                monitor.x().map_err(|e| e.to_string())?,
                monitor.y().map_err(|e| e.to_string())?,
            );
            let size = tauri::PhysicalSize::new(shot.width, shot.height);
            app.state::<Shared>()
                .lock()
                .map_err(|e| e.to_string())?
                .selection = Some(shot);
            let selector = tauri::WebviewWindowBuilder::new(
                &app,
                "selector",
                tauri::WebviewUrl::App("selector.html".into()),
            )
            .title("Select recording area")
            .decorations(false)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
            selector.set_position(position).map_err(|e| e.to_string())?;
            selector.set_size(size).map_err(|e| e.to_string())?;
            // True fullscreen removes the invisible Windows frame insets, so
            // the screenshot client area starts at the monitor's exact origin.
            selector.set_fullscreen(true).map_err(|e| e.to_string())?;
            let handle = app.clone();
            selector.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let app = handle.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let _ = finish_selection_impl(&app, None, false);
                    });
                }
            });
            selector.show().map_err(|e| e.to_string())?;
            selector.set_focus().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = finish_selection_impl(&app, None, false);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_folder() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(recorder::folder()?)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn recording_folder() -> Result<String, String> {
    Ok(recorder::folder()?.to_string_lossy().into_owned())
}
#[tauri::command]
fn open_recording_location(state: tauri::State<Shared>) -> Result<(), String> {
    let path = state
        .lock()
        .map_err(|e| e.to_string())?
        .recorder
        .saved_path()
        .map(str::to_owned)
        .ok_or("No saved recording yet")?;
    if !std::path::Path::new(&path).is_file() {
        return open_folder();
    }
    std::process::Command::new("explorer.exe")
        .arg("/select,")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let app = tauri::Builder::default()
        .manage(Shared::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(e) = toggle(&app) {
                                let _ = app.emit("recorder-error", e);
                            }
                        });
                    }
                })
                .build(),
        )
        .setup(|app| {
            // The hidden indicator must not keep the process alive after closing RecMi.
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        handle.exit(0);
                    }
                });
            }
            // Alt+F4 on the floating widget stops and saves the recording.
            if let Some(window) = app.get_webview_window("indicator") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let app = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = stop_recording(app.clone()).await {
                                let _ = app.emit("recorder-error", error);
                            }
                        });
                    }
                });
            }
            if let Err(e) = app.global_shortcut().register("F9") {
                app.state::<Shared>().lock().unwrap().shortcut_error = Some(format!(
                    "F9 unavailable: {e}. Use the recording button and the floating stop control."
                ));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_recording,
            stop_recording,
            status,
            configure,
            begin_selection,
            finish_selection,
            selection_data,
            open_folder,
            recording_folder,
            open_recording_location
        ])
        .build(tauri::generate_context!())
        .expect("Unable to launch RecMi");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            let state = app.state::<Shared>();
            if let Ok(mut state) = state.lock() {
                let _ = state.recorder.stop();
            };
        }
    });
}
