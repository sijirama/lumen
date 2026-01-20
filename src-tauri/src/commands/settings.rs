//INFO: Settings commands for Lumen
//NOTE: Handles reading and updating application settings

use crate::crypto::{decrypt_token, encrypt_token};
use crate::database::queries::{
    get_all_integrations, get_api_token, get_hotkey_config, get_integration, get_setting,
    get_user_profile, save_api_token, save_hotkey_config, save_integration, save_setting,
    save_user_profile, HotkeyConfig, Integration,
};
use crate::database::Database;
use serde::{Deserialize, Serialize};
use tauri::State;

//INFO: User profile response structure
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub display_name: String,
    pub location: Option<String>,
    pub theme: String,
}

//INFO: Hotkey config response structure
#[derive(Debug, Serialize)]
pub struct HotkeyConfigResponse {
    pub modifier_keys: Vec<String>,
    pub key: String,
    pub enabled: bool,
}

//INFO: API key status response (never returns actual key)
#[derive(Debug, Serialize)]
pub struct ApiKeyStatusResponse {
    pub provider: String,
    pub is_configured: bool,
    pub masked_key: Option<String>,
}

//INFO: Request to update user profile
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: String,
    pub location: Option<String>,
    pub theme: String,
}

//INFO: Request to update hotkey
#[derive(Debug, Deserialize)]
pub struct UpdateHotkeyRequest {
    pub modifier_keys: Vec<String>,
    pub key: String,
    pub enabled: bool,
}

//INFO: Request to update API key
#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub provider: String,
    pub api_key: String,
}

// ============================================================================
// Profile Commands
// ============================================================================

//INFO: Gets the current user profile
#[tauri::command]
pub fn get_profile(database: State<Database>) -> Result<Option<UserProfileResponse>, String> {
    let connection = database.connection.lock();

    let profile =
        get_user_profile(&connection).map_err(|e| format!("Failed to get profile: {}", e))?;

    Ok(profile.map(|p| UserProfileResponse {
        display_name: p.display_name,
        location: p.location,
        theme: p.theme,
    }))
}

//INFO: Updates the user profile
#[tauri::command]
pub fn update_profile(
    database: State<Database>,
    request: UpdateProfileRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    save_user_profile(
        &connection,
        &request.display_name,
        request.location.as_deref(),
        &request.theme,
    )
    .map_err(|e| format!("Failed to update profile: {}", e))?;

    Ok(())
}

// ============================================================================
// Hotkey Commands
// ============================================================================

//INFO: Gets the current hotkey configuration
#[tauri::command]
pub fn get_hotkey(database: State<Database>) -> Result<Option<HotkeyConfigResponse>, String> {
    let connection = database.connection.lock();

    let config =
        get_hotkey_config(&connection).map_err(|e| format!("Failed to get hotkey: {}", e))?;

    Ok(config.map(|c| HotkeyConfigResponse {
        modifier_keys: c.modifier_keys,
        key: c.key,
        enabled: c.enabled,
    }))
}

//INFO: Updates the hotkey configuration
#[tauri::command]
pub fn update_hotkey(
    database: State<Database>,
    request: UpdateHotkeyRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    let config = HotkeyConfig {
        modifier_keys: request.modifier_keys,
        key: request.key,
        enabled: request.enabled,
    };

    save_hotkey_config(&connection, &config)
        .map_err(|e| format!("Failed to update hotkey: {}", e))?;

    Ok(())
}

// ============================================================================
// API Key Commands
// ============================================================================

//INFO: Gets the status of an API key (without exposing the actual key)
#[tauri::command]
pub fn get_api_key_status(
    database: State<Database>,
    provider: String,
) -> Result<ApiKeyStatusResponse, String> {
    let connection = database.connection.lock();

    let encrypted_token = get_api_token(&connection, &provider)
        .map_err(|e| format!("Failed to get API key status: {}", e))?;

    let (is_configured, masked_key) = match encrypted_token {
        Some(encrypted) => {
            //INFO: Decrypt to get the key length for masking
            match decrypt_token(&encrypted) {
                Ok(key) => {
                    //INFO: Create masked version showing only last 4 characters
                    let masked = if key.len() > 4 {
                        format!("{}...{}", "*".repeat(8), &key[key.len() - 4..])
                    } else {
                        "*".repeat(key.len())
                    };
                    (true, Some(masked))
                }
                Err(_) => (true, Some("********".to_string())),
            }
        }
        None => (false, None),
    };

    Ok(ApiKeyStatusResponse {
        provider,
        is_configured,
        masked_key,
    })
}

//INFO: Updates an API key
#[tauri::command]
pub fn update_api_key(
    database: State<Database>,
    request: UpdateApiKeyRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    //INFO: Encrypt the API key before storing
    let encrypted_key =
        encrypt_token(&request.api_key).map_err(|e| format!("Failed to encrypt API key: {}", e))?;

    save_api_token(&connection, &request.provider, &encrypted_key, "api_key")
        .map_err(|e| format!("Failed to update API key: {}", e))?;

    Ok(())
}

// ============================================================================
// Integration Commands
// ============================================================================

//INFO: Gets all integrations
#[tauri::command]
pub fn get_integrations(database: State<Database>) -> Result<Vec<Integration>, String> {
    let connection = database.connection.lock();

    get_all_integrations(&connection).map_err(|e| format!("Failed to get integrations: {}", e))
}

//INFO: Gets a specific integration by name
#[tauri::command]
pub fn get_integration_by_name(
    database: State<Database>,
    name: String,
) -> Result<Option<Integration>, String> {
    let connection = database.connection.lock();

    get_integration(&connection, &name).map_err(|e| format!("Failed to get integration: {}", e))
}

//INFO: Updates an integration
#[tauri::command]
pub fn update_integration(
    database: State<Database>,
    integration: Integration,
) -> Result<(), String> {
    let connection = database.connection.lock();

    save_integration(&connection, &integration)
        .map_err(|e| format!("Failed to update integration: {}", e))
}

// ============================================================================
// Database Export/Import Commands
// ============================================================================

//INFO: Gets the path to the database file for export
#[tauri::command]
pub fn get_database_path(database: State<Database>) -> Result<String, String> {
    Ok(database.get_database_path().to_string_lossy().to_string())
}

//INFO: Generic setting getter
#[tauri::command]
pub fn get_app_setting(database: State<Database>, key: String) -> Result<Option<String>, String> {
    let connection = database.connection.lock();

    get_setting(&connection, &key).map_err(|e| format!("Failed to get setting: {}", e))
}

//INFO: Generic setting setter
#[tauri::command]
pub fn save_app_setting(
    database: State<Database>,
    key: String,
    value: String,
) -> Result<(), String> {
    let connection = database.connection.lock();

    save_setting(&connection, &key, &value).map_err(|e| format!("Failed to save setting: {}", e))
}
