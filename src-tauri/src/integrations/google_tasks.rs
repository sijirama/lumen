// src-tauri/src/integrations/google_tasks.rs
use crate::crypto::{decrypt_token, encrypt_token};
use crate::database::queries::{get_api_token, get_integration, save_api_token};
use crate::database::Database;
use crate::oauth::google::{GoogleAuth, GoogleTokens};
use anyhow::{anyhow, Context, Result};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleTask {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub status: String, // "needsAction" or "completed"
    pub due: Option<String>,
}

pub async fn list_tasks(database: &Database, max_results: u32) -> Result<Vec<GoogleTask>> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let client = reqwest::Client::new();

    // 1. Get default tasklist ID
    let list_url = "https://tasks.googleapis.com/tasks/v1/users/@me/lists";
    let list_response = client
        .get(list_url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .send()
        .await?;

    let lists_data: serde_json::Value = list_response.json().await?;
    let tasklist_id = lists_data["items"][0]["id"]
        .as_str()
        .ok_or_else(|| anyhow!("No tasklists found"))?;

    // 2. Fetch tasks from the first list
    let tasks_url = format!(
        "https://tasks.googleapis.com/tasks/v1/lists/{}/tasks?maxResults={}&showCompleted=false",
        tasklist_id, max_results
    );

    let tasks_response = client
        .get(&tasks_url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .send()
        .await?;

    let tasks_data: serde_json::Value = tasks_response.json().await?;
    let items = tasks_data["items"].as_array();

    if items.is_none() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();
    for item in items.unwrap() {
        let task: GoogleTask = serde_json::from_value(item.clone())?;
        tasks.push(task);
    }

    Ok(tasks)
}

pub async fn create_task(
    database: &Database,
    title: &str,
    notes: Option<&str>,
    due: Option<&str>,
) -> Result<GoogleTask> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let client = reqwest::Client::new();

    // Get default tasklist
    let list_url = "https://tasks.googleapis.com/tasks/v1/users/@me/lists";
    let list_response = client
        .get(list_url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .send()
        .await?;
    let lists_data: serde_json::Value = list_response.json().await?;
    let tasklist_id = lists_data["items"][0]["id"]
        .as_str()
        .ok_or_else(|| anyhow!("No tasklists found"))?;

    let url = format!(
        "https://tasks.googleapis.com/tasks/v1/lists/{}/tasks",
        tasklist_id
    );
    let body = json!({
        "title": title,
        "notes": notes,
        "due": due
    });

    let response = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to create task: {}", response.text().await?));
    }

    let task: GoogleTask = response.json().await?;
    Ok(task)
}

fn get_google_tokens(connection: &rusqlite::Connection) -> Result<GoogleTokens> {
    let encrypted =
        get_api_token(connection, "google")?.ok_or_else(|| anyhow!("Google tokens not found"))?;

    let decrypted = decrypt_token(&encrypted)?;
    let tokens: GoogleTokens = serde_json::from_str(&decrypted)?;
    Ok(tokens)
}

async fn refresh_google_tokens(
    database: &Database,
    current_tokens: &GoogleTokens,
) -> Result<GoogleTokens> {
    let (client_id, client_secret, refresh_token) = {
        let connection = database.connection.lock();
        let refresh_token = current_tokens
            .refresh_token
            .clone()
            .ok_or_else(|| anyhow!("No refresh token found for Google"))?;

        let integration = get_integration(&connection, "google")?
            .ok_or_else(|| anyhow!("Google integration config not found"))?;

        let config: serde_json::Value =
            serde_json::from_str(&integration.config.context("Missing config")?)?;
        let client_id = config["client_id"]
            .as_str()
            .context("Missing client_id")?
            .to_string();
        let client_secret = config["client_secret"]
            .as_str()
            .context("Missing client_secret")?
            .to_string();
        (client_id, client_secret, refresh_token)
    };

    let auth = GoogleAuth::new(client_id, client_secret);
    let mut new_tokens = auth.refresh_access_token(refresh_token).await?;

    if new_tokens.refresh_token.is_none() {
        new_tokens.refresh_token = current_tokens.refresh_token.clone();
    }

    {
        let connection = database.connection.lock();
        let tokens_json = serde_json::to_string(&new_tokens)?;
        let encrypted = encrypt_token(&tokens_json)?;
        save_api_token(&connection, "google", &encrypted, "oauth2")?;
    }

    Ok(new_tokens)
}

fn is_expired(tokens: &GoogleTokens) -> bool {
    match tokens.expires_at {
        Some(expiry) => chrono::Utc::now() + chrono::Duration::minutes(5) >= expiry,
        None => true,
    }
}
