//INFO: Lumen Library - Main entry point for the Tauri application
//NOTE: This file wires together all modules and registers Tauri commands

pub mod agent;
pub mod commands;
pub mod crypto;
pub mod database;
pub mod gemini;
pub mod integrations;
pub mod oauth;

use commands::{auth, chat, dashboard, settings, setup, vision, window};
use database::{initialize_database, Database};
use tauri::Manager;

//INFO: Main run function that initializes and starts the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        //INFO: Initialize Tauri plugins
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        //INFO: Setup hook to initialize database and other resources
        .setup(|app| {
            //INFO: Initialize the database connection
            let database = Database::new().expect("Failed to initialize database");

            //INFO: Initialize database schema (create tables if not exist)
            {
                let connection = database.connection.lock();
                initialize_database(&connection).expect("Failed to initialize database schema");
            }

            //INFO: Store database in app state for access from commands
            let db_clone = database.clone();
            app.manage(database);

            // Start proactive background agent
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                agent::proactive::start_proactive_agent(app_handle, db_clone).await;
            });

            //INFO: Setup global hotkey listener
            let _ = setup_global_hotkey(app);

            //INFO: Setup system tray
            let _ = setup_system_tray(app);

            //INFO: Auto-show main window unless --minimized flag is present
            let args: Vec<String> = std::env::args().collect();
            let is_minimized = args.iter().any(|arg| arg == "--minimized");

            if !is_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            Ok(())
        })
        //INFO: Handle window events to prevent app from closing when windows are closed
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                //INFO: Don't close the window, just hide it
                //NOTE: This keeps the app running in the background
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        //INFO: Register all Tauri commands
        .invoke_handler(tauri::generate_handler![
            // Setup commands
            setup::check_setup_status,
            setup::setup_save_profile,
            setup::setup_save_hotkey,
            setup::setup_save_api_key,
            setup::test_gemini_api_key,
            setup::setup_save_integration,
            setup::complete_setup,
            // Settings commands
            settings::get_profile,
            settings::update_profile,
            settings::get_hotkey,
            settings::update_hotkey,
            settings::get_api_key_status,
            settings::update_api_key,
            settings::get_integrations,
            settings::get_integration_by_name,
            settings::update_integration,
            settings::get_database_path,
            settings::get_app_setting,
            settings::save_app_setting,
            // Chat commands
            chat::send_chat_message,
            chat::get_chat_history,
            chat::clear_chat_history,
            // Window commands
            window::show_overlay,
            window::hide_overlay,
            window::toggle_overlay,
            window::is_overlay_visible,
            window::show_main_window,
            window::hide_main_window,
            window::open_path,
            // Dashboard commands
            dashboard::get_dashboard_briefing,
            dashboard::refresh_dashboard_briefing,
            // Auth commands
            auth::get_google_auth_status,
            auth::save_google_config,
            auth::start_google_auth,
            // Vision commands
            vision::capture_primary_screen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

//INFO: Sets up the global hotkey listener
//NOTE: Uses the hotkey configured by the user to toggle the overlay
fn setup_global_hotkey(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    //INFO: Get the database to read hotkey configuration
    let database = app.state::<Database>();
    let connection = database.connection.lock();

    //INFO: Try to get the user's configured hotkey
    let hotkey_config = database::queries::get_hotkey_config(&connection)
        .ok()
        .flatten();

    drop(connection); // Release the lock before async operations

    //INFO: Default to Super+L if no hotkey is configured
    let shortcut_str = if let Some(config) = hotkey_config {
        if config.enabled {
            //INFO: Build shortcut string from modifier keys and key
            let modifiers = config.modifier_keys.join("+");
            if modifiers.is_empty() {
                config.key
            } else {
                format!("{}+{}", modifiers, config.key)
            }
        } else {
            return Ok(()); // Hotkey disabled, don't register
        }
    } else {
        "Super+L".to_string() // Default hotkey
    };

    //INFO: Parse and register the shortcut
    if let Ok(shortcut) = shortcut_str.parse::<Shortcut>() {
        let app_handle = app.app_handle().clone();

        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    //INFO: Toggle overlay visibility on the main thread to avoid X11 crashes
                    let app_handle_clone = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        tauri::async_runtime::block_on(async move {
                            let _ = window::toggle_overlay(app_handle_clone).await;
                        });
                    });
                }
            })?;

        //INFO: Register the shortcut
        app.global_shortcut().register(shortcut)?;
    }

    Ok(())
}

//INFO: Sets up the system tray icon and menu
fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    //INFO: Create tray menu items
    let show_item = MenuItem::with_id(app, "show", "Show Lumen", true, None::<&str>)?;
    let chat_item = MenuItem::with_id(app, "chat", "Open Chat", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    //INFO: Build the tray menu
    let menu = Menu::with_items(app, &[&show_item, &chat_item, &quit_item])?;

    //INFO: Build the tray icon
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    //INFO: Show the main window
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "chat" => {
                    //INFO: Toggle the overlay
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = window::toggle_overlay(app_handle).await;
                    });
                }
                "quit" => {
                    //INFO: Quit the application
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    //INFO: Left click toggles overlay
                    let app = tray.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = window::toggle_overlay(app).await;
                    });
                }
            }
        })
        .build(app)?;

    Ok(())
}
