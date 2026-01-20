// src-tauri/src/integrations/google_gmail.rs
use crate::crypto::{decrypt_token, encrypt_token};
use crate::database::queries::{get_api_token, get_integration, save_api_token};
use crate::database::Database;
use crate::oauth::google::{GoogleAuth, GoogleTokens};
use anyhow::{anyhow, Context, Result};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub snippet: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<String>,
}

pub async fn send_email(database: &Database, to: &str, subject: &str, body: &str) -> Result<()> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let url = "https://gmail.googleapis.com/gmail/v1/users/me/messages/send";

    // Build raw email (simplified RFC 822)
    let email_raw = format!(
        "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{}",
        to, subject, body
    );

    // Base64Url encode it
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(email_raw);

    let payload = serde_json::json!({
        "raw": encoded
    });

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .json(&payload)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        tokens = refresh_google_tokens(database, &tokens).await?;
        let response = client
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to send email: {}", response.text().await?));
        }
    } else if !response.status().is_success() {
        return Err(anyhow!("Failed to send email: {}", response.text().await?));
    }

    Ok(())
}

use base64::Engine;

pub async fn fetch_recent_emails(
    database: &Database,
    max_results: u32,
) -> Result<Vec<GmailMessage>> {
    fetch_recent_emails_with_query(database, max_results, None).await
}

pub async fn fetch_recent_emails_with_query(
    database: &Database,
    max_results: u32,
    query: Option<&str>,
) -> Result<Vec<GmailMessage>> {
    let mut tokens = {
        let connection = database.connection.lock();
        get_google_tokens(&connection)?
    };

    // Check if expired and refresh if needed
    if is_expired(&tokens) {
        tokens = refresh_google_tokens(database, &tokens).await?;
    }

    let client = reqwest::Client::new();

    // Build query - default to unread inbox, but allow custom queries
    let q = query.unwrap_or("is:unread inbox");
    let encoded_q = urlencoding::encode(q);

    // 1. Get list of message IDs
    let list_url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages?maxResults={}&q={}",
        max_results, encoded_q
    );

    let list_response = client
        .get(&list_url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .send()
        .await?;

    if list_response.status() == reqwest::StatusCode::UNAUTHORIZED {
        // Try refresh once more
        tokens = refresh_google_tokens(database, &tokens).await?;
        return Box::pin(fetch_recent_emails_with_tokens(
            database,
            &tokens,
            max_results,
        ))
        .await;
    }

    let list_data: serde_json::Value = list_response.json().await?;
    let message_summaries = list_data["messages"].as_array();

    if message_summaries.is_none() {
        return Ok(Vec::new());
    }

    let mut emails = Vec::new();
    for msg_ref in message_summaries.unwrap() {
        let id = msg_ref["id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing message id"))?;

        // 2. Fetch full message details
        let detail_url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
            id
        );
        let detail_response = client
            .get(&detail_url)
            .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
            .send()
            .await?;

        let detail_data: serde_json::Value = detail_response.json().await?;

        let mut subject = None;
        let mut from = None;
        let mut date = None;

        if let Some(headers) = detail_data["payload"]["headers"].as_array() {
            for header in headers {
                match header["name"].as_str() {
                    Some("Subject") => subject = header["value"].as_str().map(|s| s.to_string()),
                    Some("From") => from = header["value"].as_str().map(|s| s.to_string()),
                    Some("Date") => date = header["value"].as_str().map(|s| s.to_string()),
                    _ => {}
                }
            }
        }

        emails.push(GmailMessage {
            id: id.to_string(),
            thread_id: detail_data["threadId"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            snippet: detail_data["snippet"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            subject,
            from,
            date,
        });
    }

    Ok(emails)
}

// Helper to avoid recursive async issues
async fn fetch_recent_emails_with_tokens(
    _database: &Database,
    tokens: &GoogleTokens,
    max_results: u32,
) -> Result<Vec<GmailMessage>> {
    let client = reqwest::Client::new();
    let list_url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages?maxResults={}&q=is:unread inbox",
        max_results
    );

    let list_response = client
        .get(&list_url)
        .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
        .send()
        .await?;

    let list_data: serde_json::Value = list_response.json().await?;
    let message_summaries = list_data["messages"].as_array();

    if message_summaries.is_none() {
        return Ok(Vec::new());
    }

    let mut emails = Vec::new();
    for msg_ref in message_summaries.unwrap() {
        let id = msg_ref["id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing message id"))?;
        let detail_url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
            id
        );
        let detail_response = client
            .get(&detail_url)
            .header(AUTHORIZATION, format!("Bearer {}", tokens.access_token))
            .send()
            .await?;

        let detail_data: serde_json::Value = detail_response.json().await?;

        let mut subject = None;
        let mut from = None;
        let mut date = None;

        if let Some(headers) = detail_data["payload"]["headers"].as_array() {
            for header in headers {
                match header["name"].as_str() {
                    Some("Subject") => subject = header["value"].as_str().map(|s| s.to_string()),
                    Some("From") => from = header["value"].as_str().map(|s| s.to_string()),
                    Some("Date") => date = header["value"].as_str().map(|s| s.to_string()),
                    _ => {}
                }
            }
        }

        emails.push(GmailMessage {
            id: id.to_string(),
            thread_id: detail_data["threadId"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            snippet: detail_data["snippet"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            subject,
            from,
            date,
        });
    }

    Ok(emails)
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
