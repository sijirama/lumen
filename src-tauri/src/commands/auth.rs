// src-tauri/src/commands/auth.rs
use crate::crypto::encrypt_token;
use crate::database::queries::{get_integration, save_api_token, save_integration, Integration};
use crate::database::Database;
use crate::oauth::google::GoogleAuth;
use serde_json::json;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_google_auth_status(database: State<'_, Database>) -> Result<bool, String> {
    let connection = database.connection.lock();
    crate::database::queries::has_api_token(&connection, "google").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_google_config(
    database: State<'_, Database>,
    client_id: String,
    client_secret: String,
) -> Result<(), String> {
    let connection = database.connection.lock();

    let config = json!({
        "client_id": client_id,
        "client_secret": client_secret
    })
    .to_string();

    let integration = Integration {
        name: "google".to_string(),
        enabled: false,
        config: Some(config),
        last_sync: None,
        status: "configured".to_string(),
    };

    save_integration(&connection, &integration).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_google_auth(
    handle: AppHandle,
    database: State<'_, Database>,
) -> Result<String, String> {
    // 1. Get Google Client ID and Secret from integrations
    let (client_id, client_secret) = {
        let connection = database.connection.lock();
        let integration = get_integration(&connection, "google")
            .map_err(|e| e.to_string())?
            .ok_or("Google integration not configured. Please enter Client ID and Secret first.")?;

        let config: serde_json::Value =
            serde_json::from_str(&integration.config.clone().unwrap_or_default())
                .map_err(|_| "Invalid Google integration config")?;

        let id = config["client_id"]
            .as_str()
            .ok_or("Missing client_id")?
            .to_string();
        let secret = config["client_secret"]
            .as_str()
            .ok_or("Missing client_secret")?
            .to_string();
        (id, secret)
    };

    let auth = GoogleAuth::new(client_id.clone(), client_secret.clone());
    let (url, state) = auth.start_auth_flow().await.map_err(|e| e.to_string())?;

    // Open browser using tauri-plugin-opener
    let opener_handle = handle.clone();
    let url_clone = url.clone();
    tauri::async_runtime::spawn(async move {
        // In Tauri 2.0, open is a command in the plugin. We can use it via handle.
        let _ = tauri_plugin_opener::OpenerExt::opener(&opener_handle)
            .open_url(url_clone, None::<String>);
    });

    // Start local server to catch code (blocks this command until code is received or fails)
    let code = tauri::async_runtime::spawn_blocking(move || auth.listen_for_code(state))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // Exchange code for tokens
    let auth_exchange = GoogleAuth::new(client_id, client_secret);
    let tokens = auth_exchange
        .exchange_code(code)
        .await
        .map_err(|e| e.to_string())?;

    // Save tokens (encrypted)
    {
        let connection = database.connection.lock();
        let tokens_json = serde_json::to_string(&tokens).map_err(|e| e.to_string())?;
        let encrypted = encrypt_token(&tokens_json).map_err(|e| e.to_string())?;
        save_api_token(&connection, "google", &encrypted, "oauth2").map_err(|e| e.to_string())?;

        // Update integration status
        let mut integration = get_integration(&connection, "google").unwrap().unwrap();
        integration.enabled = true;
        integration.status = "connected".to_string();
        save_integration(&connection, &integration).map_err(|e| e.to_string())?;
    }

    Ok("Connected successfully".to_string())
}
