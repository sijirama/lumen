use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};

//INFO: Cache for the screenshot we are snipping
static LAST_SCREENSHOT: Mutex<Option<screenshots::image::DynamicImage>> = Mutex::new(None);

#[tauri::command]
pub async fn capture_primary_screen() -> Result<String, String> {
    use std::io::Cursor;
    let start = Instant::now();
    let screens = Screen::all().map_err(|e| e.to_string())?;

    if let Some(screen) = screens.first() {
        let capture = screen.capture().map_err(|e| e.to_string())?;

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        capture
            .write_to(&mut cursor, screenshots::image::ImageFormat::Png)
            .map_err(|e: screenshots::image::ImageError| e.to_string())?;

        let b64 = general_purpose::STANDARD.encode(buffer);
        println!("Captured screen in {:?}", start.elapsed());
        Ok(b64)
    } else {
        Err("No screens found".to_string())
    }
}

//INFO: Starts the snipping workflow
#[tauri::command]
pub async fn start_snipping(app: AppHandle) -> Result<(), String> {
    // 1. Hide Overlay
    if let Some(overlay) = app.get_webview_window("overlay") {
        overlay.hide().map_err(|e| e.to_string())?;
    }

    // 2. Wait for animation/hide (essential for Linux/compositors)
    tokio::time::sleep(Duration::from_millis(250)).await;

    // 3. Capture Screen
    let screens = Screen::all().map_err(|e| e.to_string())?;
    let screen = screens.first().ok_or("No screen found")?;
    let image = screen.capture().map_err(|e| e.to_string())?;

    // 4. Cache it
    {
        let mut cache = LAST_SCREENSHOT.lock().map_err(|_| "Failed to lock cache")?;
        *cache = Some(screenshots::image::DynamicImage::ImageRgba8(image));
    }

    // 5. Show Snipper Window
    // 5. Show Snipper Window
    if let Some(snipper) = app.get_webview_window("snipper") {
        //INFO: Manually force fullscreen size to ensure coverage
        if let Ok(Some(monitor)) = snipper.primary_monitor() {
            let size = monitor.size();
            let pos = monitor.position();

            // disable resizable before setting size/pos might help on some WMs
            let _ = snipper.set_resizable(true);
            let _ = snipper.set_position(*pos);
            let _ = snipper.set_size(*size);
            let _ = snipper.set_resizable(false);
        }

        snipper.show().map_err(|e| e.to_string())?;
        snipper.set_focus().map_err(|e| e.to_string())?;
        snipper.set_always_on_top(true).map_err(|e| e.to_string())?;
    }

    Ok(())
}

//INFO: Closes snipper and re-shows overlay
#[tauri::command]
pub async fn close_snipper(app: AppHandle) -> Result<(), String> {
    if let Some(snipper) = app.get_webview_window("snipper") {
        snipper.hide().map_err(|e| e.to_string())?;
    }

    //INFO: Small delay to let WM process the fullscreen exit
    tokio::time::sleep(Duration::from_millis(150)).await;

    if let Some(overlay) = app.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        //INFO: Ensure overlay returns to its correct position
        if let Err(e) = crate::commands::window::position_overlay_bottom_left(&overlay) {
            println!("Failed to position overlay: {}", e);
        }
        overlay.set_focus().map_err(|e| e.to_string())?;
    }

    // Clear cache
    {
        if let Ok(mut cache) = LAST_SCREENSHOT.lock() {
            *cache = None;
        }
    }
    Ok(())
}

//INFO: Crops the cached screenshot and emits it
#[tauri::command]
pub async fn capture_region(
    app: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    use screenshots::image::GenericImageView;
    use std::io::Cursor;

    // 1. Get cached image
    let mut image = {
        let cache = LAST_SCREENSHOT.lock().map_err(|_| "Failed to lock cache")?;
        cache.clone().ok_or("No screenshot in cache")?
    };

    // 2. Handle DPI / Scaling logic
    // The screenshot is in physical pixels. The x, y, width, height from frontend are CSS pixels.
    // We need to scale them.
    // However, on Linux, `screenshots` crate usually returns physical pixels.
    // And Tauri's `AppHandler` or Window can tell us the scale factor.

    let scale_factor = if let Some(snipper) = app.get_webview_window("snipper") {
        snipper.scale_factor().unwrap_or(1.0)
    } else if let Some(main) = app.get_webview_window("main") {
        main.scale_factor().unwrap_or(1.0)
    } else {
        1.0
    };

    // Convert CSS pixels to Physical pixels
    let px = (x * scale_factor) as u32;
    let py = (y * scale_factor) as u32;
    let pwidth = (width * scale_factor) as u32;
    let pheight = (height * scale_factor) as u32;

    // Safe crop bounds
    let img_width = image.width();
    let img_height = image.height();

    // Ensure we don't crop out of bounds (can happen with multiple monitors or rounding)
    let cx = px.min(img_width - 1);
    let cy = py.min(img_height - 1);
    let cw = pwidth.min(img_width - cx);
    let ch = pheight.min(img_height - cy);

    if cw == 0 || ch == 0 {
        return close_snipper(app).await;
    }

    let cropped = image.crop(cx, cy, cw, ch);

    // 3. Encode to Base64
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    cropped
        .write_to(&mut cursor, screenshots::image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let b64 = general_purpose::STANDARD.encode(buffer);

    // 4. Emit to overlay
    app.emit("snipped-image", b64).map_err(|e| e.to_string())?;

    // 5. Close Window
    close_snipper(app).await
}
