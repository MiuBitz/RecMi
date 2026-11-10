use tauri::{AppHandle, Manager, WebviewWindow};

#[cfg(windows)]
mod affinity {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: *mut c_void, affinity: u32) -> i32;
        fn GetWindowDisplayAffinity(hwnd: *mut c_void, affinity: *mut u32) -> i32;
    }
    // Unlike WDA_MONITOR (which leaves a black rectangle), this removes the
    // window from capture and reveals the desktop underneath it.
    pub(super) fn exclude(hwnd: *mut c_void) -> Result<(), String> {
        let mut actual = 0;
        unsafe {
            if SetWindowDisplayAffinity(hwnd, 0x11) == 0
                || GetWindowDisplayAffinity(hwnd, &mut actual) == 0
                || actual != 0x11
            {
                return Err(format!(
                    "Cannot exclude the recording indicator from capture: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(())
    }
}

pub fn prepare(app: &AppHandle) -> Result<WebviewWindow, String> {
    let window = app
        .get_webview_window("indicator")
        .ok_or("Recording indicator unavailable")?;
    #[cfg(windows)]
    affinity::exclude(window.hwnd().map_err(|e| e.to_string())?.0 as _)?;
    #[cfg(not(windows))]
    return Err("The recording indicator requires Windows capture exclusion".into());
    if let Some(monitor) = app.primary_monitor().map_err(|e| e.to_string())? {
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let margin = (16.0 * monitor.scale_factor()) as i32;
        window
            .set_position(tauri::PhysicalPosition::new(
                monitor.position().x + monitor.size().width as i32 - size.width as i32 - margin,
                monitor.position().y + margin,
            ))
            .map_err(|e| e.to_string())?;
    }
    Ok(window)
}

pub fn restore(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("indicator") {
        let _ = window.hide();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::affinity;
    use std::{ffi::c_void, process::Command, ptr};

    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: *mut c_void,
            menu: *mut c_void,
            instance: *mut c_void,
            param: *const c_void,
        ) -> *mut c_void;
        fn UpdateWindow(hwnd: *mut c_void) -> i32;
        fn SetWindowPos(
            hwnd: *mut c_void,
            after: *mut c_void,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn DestroyWindow(hwnd: *mut c_void) -> i32;
    }
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmFlush() -> i32;
    }

    struct TestWindow(*mut c_void);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    fn window(x: i32, y: i32, color_style: u32) -> TestWindow {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                8,
                class.as_ptr(),
                ptr::null(),
                0x90000000 | color_style,
                x,
                y,
                160,
                120,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        assert!(!hwnd.is_null(), "Cannot create capture test window");
        unsafe {
            SetWindowPos(hwnd, -1isize as *mut c_void, x, y, 160, 120, 0x40);
            UpdateWindow(hwnd);
            DwmFlush();
        }
        TestWindow(hwnd)
    }
    fn capture_level(x: i32, y: i32) -> f64 {
        unsafe {
            DwmFlush();
        }
        let output = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "gdigrab",
                "-framerate",
                "30",
                "-offset_x",
                &(x + 20).to_string(),
                "-offset_y",
                &(y + 20).to_string(),
                "-video_size",
                "100x80",
                "-draw_mouse",
                "0",
                "-i",
                "desktop",
                "-frames:v",
                "1",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 100 * 80 * 3);
        let level =
            output.stdout.iter().map(|v| *v as f64).sum::<f64>() / output.stdout.len() as f64;
        println!("Captured level: {level}");
        level
    }
    #[test]
    #[ignore = "briefly shows two test rectangles and verifies FFmpeg sees through the excluded one"]
    fn indicator_exclusion_reveals_desktop_in_ffmpeg() {
        let monitor = crate::recorder::primary().unwrap();
        let (x, y) = (monitor.x().unwrap() + 40, monitor.y().unwrap() + 40);
        let _background = window(x, y, 6); // SS_WHITERECT
        assert!(
            capture_level(x, y) > 245.0,
            "Test background is not visible"
        );
        let overlay = window(x, y, 4); // SS_BLACKRECT
        assert!(capture_level(x, y) < 200.0, "Test overlay is not visible");
        affinity::exclude(overlay.0).unwrap();
        assert!(
            capture_level(x, y) > 245.0,
            "Excluded overlay leaves pixels or a black rectangle in the video"
        );
        println!("FFmpeg captures the background underneath the excluded window");
    }
}
