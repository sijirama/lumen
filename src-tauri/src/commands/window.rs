//INFO: Window management commands for Lumen
//NOTE: Handles overlay window show/hide and positioning

use tauri::{Manager, WebviewWindow};

//INFO: Shows the overlay window
#[tauri::command]
pub async fn show_overlay(app: tauri::AppHandle) -> Result<(), String> {
    //INFO: Get the overlay window by its label
    if let Some(overlay_window) = app.get_webview_window("overlay") {
        //INFO: Show first, then position
        overlay_window
            .show()
            .map_err(|e| format!("Failed to show overlay: {}", e))?;

        //INFO: Make it visible on all workspaces (Sticky)
        let _ = overlay_window.set_visible_on_all_workspaces(true);
        let _ = position_overlay_bottom_left(&overlay_window);
        overlay_window
            .set_focus()
            .map_err(|e| format!("Failed to focus overlay: {}", e))?;

        Ok(())
    } else {
        Err("Overlay window not found".to_string())
    }
}

//INFO: Hides the overlay window
#[tauri::command]
pub async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(overlay_window) = app.get_webview_window("overlay") {
        overlay_window
            .hide()
            .map_err(|e| format!("Failed to hide overlay: {}", e))?;
        Ok(())
    } else {
        Err("Overlay window not found".to_string())
    }
}

//INFO: Toggles the overlay window visibility
#[tauri::command]
pub async fn toggle_overlay(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(overlay_window) = app.get_webview_window("overlay") {
        let is_visible = overlay_window
            .is_visible()
            .map_err(|e| format!("Failed to check visibility: {}", e))?;

        if is_visible {
            overlay_window
                .hide()
                .map_err(|e| format!("Failed to hide overlay: {}", e))?;
            Ok(false)
        } else {
            //INFO: Show first to ensure window resources are allocated
            overlay_window
                .show()
                .map_err(|e| format!("Failed to show overlay: {}", e))?;

            //INFO: Make it visible on all workspaces (Sticky)
            let _ = overlay_window.set_visible_on_all_workspaces(true);

            //INFO: Then position it (ignore errors to prevent crash)
            let _ = position_overlay_bottom_left(&overlay_window);

            overlay_window
                .set_focus()
                .map_err(|e| format!("Failed to focus overlay: {}", e))?;
            Ok(true)
        }
    } else {
        Err("Overlay window not found".to_string())
    }
}

//INFO: Checks if the overlay is currently visible
#[tauri::command]
pub async fn is_overlay_visible(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(overlay_window) = app.get_webview_window("overlay") {
        overlay_window
            .is_visible()
            .map_err(|e| format!("Failed to check visibility: {}", e))
    } else {
        Err("Overlay window not found".to_string())
    }
}

//INFO: Resizes and re-positions the overlay based on the view
#[tauri::command]
pub async fn resize_overlay(app: tauri::AppHandle, view: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let (width, height) = match view.as_str() {
            "calendar" => (400.0, 850.0),
            "calendar-collapsed" => (400.0, 600.0),
            _ => (400.0, 520.0),
        };

        // 1. Set Size
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| format!("Failed to set size: {}", e))?;

        // 2. Medium sleep to let WM catch up (critical for Linux stability)
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        // 3. Re-position to keep bottom-left fixed
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let monitor_size = monitor.size();
            let monitor_position = monitor.position();
            let window_size = window
                .outer_size()
                .map_err(|e| format!("Failed to get window size: {}", e))?;

            let padding = 4;
            let x_position = monitor_position.x + padding;
            let y_position = monitor_position.y + (monitor_size.height as i32)
                - (window_size.height as i32)
                - padding;

            window
                .set_position(tauri::PhysicalPosition::new(x_position, y_position))
                .map_err(|e| format!("Failed to set position: {}", e))?;

            // Ensure window is focused after resize
            let _ = window.set_focus();
        }
    }
    Ok(())
}

//INFO: Command wrapper for positioning the overlay
#[tauri::command]
pub async fn position_overlay_bottom_left_command(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        position_overlay_bottom_left(&window)?;
    }
    Ok(())
}

//INFO: Positions the overlay window at the bottom-left of the screen
pub fn position_overlay_bottom_left(window: &WebviewWindow) -> Result<(), String> {
    //INFO: Get the primary monitor's dimensions
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let monitor_size = monitor.size();
        let monitor_position = monitor.position();

        //INFO: Get the overlay window size
        let window_size = window
            .outer_size()
            .map_err(|e| format!("Failed to get window size: {}", e))?;

        //INFO: Calculate position for bottom-left corner with minimal padding
        let padding = 4;
        let x_position = monitor_position.x + padding;
        let y_position = monitor_position.y + (monitor_size.height as i32)
            - (window_size.height as i32)
            - padding;

        //INFO: Set the window position
        window
            .set_position(tauri::PhysicalPosition::new(x_position, y_position))
            .map_err(|e| format!("Failed to set position: {}", e))?;
    }

    Ok(())
}

//INFO: Shows the main application window
#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        main_window
            .show()
            .map_err(|e| format!("Failed to show main window: {}", e))?;
        main_window
            .set_focus()
            .map_err(|e| format!("Failed to focus main window: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

//INFO: Hides the main application window (minimize to tray)
#[tauri::command]
pub async fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        main_window
            .hide()
            .map_err(|e| format!("Failed to hide main window: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

//INFO: Opens a path using the system's default application
#[tauri::command]
pub async fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| e.to_string())
}
