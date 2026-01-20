//INFO: Setup wizard commands for Lumen
//NOTE: Handles the first-run setup flow

use crate::crypto::encrypt_token;
use crate::database::queries::{
    get_user_profile, is_setup_complete, mark_setup_complete, save_api_token, save_hotkey_config,
    save_integration, save_user_profile, HotkeyConfig, Integration,
};
use crate::database::Database;
use serde::{Deserialize, Serialize};
use tauri::State;

//INFO: Response for checking if setup is complete
#[derive(Debug, Serialize)]
pub struct SetupStatusResponse {
    pub setup_complete: bool,
    pub user_profile: Option<UserProfileResponse>,
}

//INFO: Response structure for user profile
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub display_name: String,
    pub location: Option<String>,
    pub theme: String,
}

//INFO: Request structure for saving user profile during setup
#[derive(Debug, Deserialize)]
pub struct SaveProfileRequest {
    pub display_name: String,
    pub location: Option<String>,
    pub theme: String,
}

//INFO: Request structure for saving hotkey during setup
#[derive(Debug, Deserialize)]
pub struct SaveHotkeyRequest {
    pub modifier_keys: Vec<String>,
    pub key: String,
}

//INFO: Request structure for saving API key during setup
#[derive(Debug, Deserialize)]
pub struct SaveApiKeyRequest {
    pub provider: String,
    pub api_key: String,
}

//INFO: Request structure for saving integration config during setup
#[derive(Debug, Deserialize)]
pub struct SaveIntegrationRequest {
    pub name: String,
    pub enabled: bool,
    pub config: Option<String>,
}

//INFO: Checks if the setup wizard has been completed
#[tauri::command]
pub fn check_setup_status(database: State<Database>) -> Result<SetupStatusResponse, String> {
    let connection = database.connection.lock();

    let setup_complete = is_setup_complete(&connection)
        .map_err(|e| format!("Failed to check setup status: {}", e))?;

    let user_profile = if setup_complete {
        get_user_profile(&connection)
            .map_err(|e| format!("Failed to get user profile: {}", e))?
            .map(|p| UserProfileResponse {
                display_name: p.display_name,
                location: p.location,
                theme: p.theme,
            })
    } else {
        None
    };

    Ok(SetupStatusResponse {
        setup_complete,
        user_profile,
    })
}

//INFO: Saves the user profile during setup
#[tauri::command]
pub fn setup_save_profile(
    database: State<Database>,
    request: SaveProfileRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    save_user_profile(
        &connection,
        &request.display_name,
        request.location.as_deref(),
        &request.theme,
    )
    .map_err(|e| format!("Failed to save profile: {}", e))?;

    Ok(())
}

//INFO: Saves the hotkey configuration during setup
#[tauri::command]
pub fn setup_save_hotkey(
    database: State<Database>,
    request: SaveHotkeyRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    let config = HotkeyConfig {
        modifier_keys: request.modifier_keys,
        key: request.key,
        enabled: true,
    };

    save_hotkey_config(&connection, &config)
        .map_err(|e| format!("Failed to save hotkey: {}", e))?;

    Ok(())
}

//INFO: Saves an API key during setup (encrypted)
#[tauri::command]
pub fn setup_save_api_key(
    database: State<Database>,
    request: SaveApiKeyRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    //INFO: Encrypt the API key before storing
    let encrypted_key =
        encrypt_token(&request.api_key).map_err(|e| format!("Failed to encrypt API key: {}", e))?;

    save_api_token(&connection, &request.provider, &encrypted_key, "api_key")
        .map_err(|e| format!("Failed to save API key: {}", e))?;

    Ok(())
}

//INFO: Tests if the Gemini API key is valid
#[tauri::command]
pub async fn test_gemini_api_key(api_key: String) -> Result<bool, String> {
    use crate::gemini::GeminiClient;

    let client = GeminiClient::new(api_key);
    let is_valid = client
        .test_connection()
        .await
        .map_err(|e| format!("API test failed: {}", e))?;

    Ok(is_valid)
}

//INFO: Saves an integration configuration during setup
#[tauri::command]
pub fn setup_save_integration(
    database: State<Database>,
    request: SaveIntegrationRequest,
) -> Result<(), String> {
    let connection = database.connection.lock();

    let integration = Integration {
        name: request.name,
        enabled: request.enabled,
        config: request.config,
        last_sync: None,
        status: if request.enabled {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        },
    };

    save_integration(&connection, &integration)
        .map_err(|e| format!("Failed to save integration: {}", e))?;

    Ok(())
}

//INFO: Marks the setup wizard as complete
#[tauri::command]
pub fn complete_setup(database: State<Database>) -> Result<(), String> {
    let connection = database.connection.lock();

    mark_setup_complete(&connection).map_err(|e| format!("Failed to complete setup: {}", e))?;

    Ok(())
}
