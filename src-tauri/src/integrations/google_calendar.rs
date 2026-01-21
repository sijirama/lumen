// src-tauri/src/integrations/google_calendar.rs
use crate::crypto::{decrypt_token, encrypt_token};
use crate::database::queries::{get_api_token, get_integration, save_api_token};
use crate::database::Database;
use crate::oauth::google::{GoogleAuth, GoogleTokens};
use anyhow::{anyhow, Context, Result};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleCalendarEvent {
    pub id: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub start: GoogleDateTime,
    pub end: GoogleDateTime,
    pub location: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleDateTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    pub date: Option<String>,
}

pub async fn fetch_google_calendar_events(
    database: &Database,
    time_min: &str, // RFC3339
    time_max: &str, // RFC3339
) -> Result<Vec<GoogleCalendarEvent>> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    // Check if expired and refresh if needed
    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

    let params = [
        ("timeMin", time_min),
        ("timeMax", time_max),
        ("singleEvents", "true"),
        ("orderBy", "startTime"),
    ];

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .query(&params)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        // Try refresh once more even if we thought it was valid
        tokens = refresh_google_tokens(database, &tokens).await?;
        let response = client
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
            .query(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!(
                "Google Calendar API error after refresh: {}",
                error_text
            ));
        }

        let data: serde_json::Value = response.json().await?;
        parse_google_events(data)
    } else {
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Google Calendar API error: {}", error_text));
        }
        let data: serde_json::Value = response.json().await?;
        parse_google_events(data)
    }
}

pub async fn create_calendar_event(
    database: &Database,
    summary: &str,
    description: Option<&str>,
    start_time: &str, // RFC3339
    end_time: &str,   // RFC3339
    location: Option<&str>,
) -> Result<GoogleCalendarEvent> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

    let event_body = json!({
        "summary": summary,
        "description": description,
        "location": location,
        "start": { "dateTime": start_time },
        "end": { "dateTime": end_time }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .json(&event_body)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        tokens = refresh_google_tokens(database, &tokens).await?;
        let response = client
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
            .json(&event_body)
            .send()
            .await?;

        let event: GoogleCalendarEvent = response.json().await?;
        Ok(event)
    } else {
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to create calendar event: {}", error_text));
        }
        let event: GoogleCalendarEvent = response.json().await?;
        Ok(event)
    }
}

use serde_json::json;

fn parse_google_events(data: serde_json::Value) -> Result<Vec<GoogleCalendarEvent>> {
    let items = data["items"]
        .as_array()
        .ok_or_else(|| anyhow!("No items in calendar response: {:?}", data))?;
    let mut events = Vec::new();

    for item in items {
        let event: GoogleCalendarEvent = serde_json::from_value(item.clone())?;
        events.push(event);
    }

    Ok(events)
}

fn get_google_tokens(connection: &rusqlite::Connection) -> Result<GoogleTokens> {
    let encrypted = get_api_token(connection, "google")?
        .ok_or_else(|| anyhow!("Google tokens not found. Please connect Google first."))?;

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

    // If the refresh response didn't include a new refresh token, keep the old one
    if new_tokens.refresh_token.is_none() {
        new_tokens.refresh_token = current_tokens.refresh_token.clone();
    }

    // Save back to DB
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
