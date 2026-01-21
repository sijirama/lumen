//INFO: Database query functions for Lumen
//NOTE: All CRUD operations for the various tables

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

//INFO: User profile data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub display_name: String,
    pub location: Option<String>,
    pub theme: String,
    pub created_at: String,
    pub updated_at: String,
}

//INFO: Hotkey configuration data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    pub modifier_keys: Vec<String>,
    pub key: String,
    pub enabled: bool,
}

//INFO: Chat message data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: Option<i64>,
    pub role: String,
    pub content: String,
    pub image_data: Option<String>,
    pub created_at: String,
    pub session_id: Option<String>,
}

//INFO: Calendar event data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub all_day: bool,
}

//INFO: Integration data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Integration {
    pub name: String,
    pub enabled: bool,
    pub config: Option<String>,
    pub last_sync: Option<String>,
    pub status: String,
}

//INFO: Briefing summary data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BriefingSummary {
    pub id: i32,
    pub content: String,
    pub data_hash: String,
    pub audio_data: Option<Vec<u8>>,
    pub created_at: String,
    pub is_final_of_day: bool,
}

// ============================================================================
// User Profile Queries
// ============================================================================

//INFO: Checks if the setup wizard has been completed
pub fn is_setup_complete(connection: &Connection) -> Result<bool> {
    let result: Option<i32> = connection
        .query_row(
            "SELECT completed FROM setup_status WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to check setup status")?;

    Ok(result.unwrap_or(0) == 1)
}

//INFO: Marks the setup wizard as completed
pub fn mark_setup_complete(connection: &Connection) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT OR REPLACE INTO setup_status (id, completed, completed_at) VALUES (1, 1, ?1)",
            params![now],
        )
        .context("Failed to mark setup as complete")?;
    Ok(())
}

//INFO: Gets the user profile from the database
//NOTE: Returns None if no profile exists (first run)
pub fn get_user_profile(connection: &Connection) -> Result<Option<UserProfile>> {
    let result = connection
        .query_row(
            "SELECT display_name, location, theme, created_at, updated_at FROM user_profile WHERE id = 1",
            [],
            |row| {
                Ok(UserProfile {
                    display_name: row.get(0)?,
                    location: row.get(1)?,
                    theme: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()
        .context("Failed to query user profile")?;

    Ok(result)
}

//INFO: Saves or updates the user profile
pub fn save_user_profile(
    connection: &Connection,
    display_name: &str,
    location: Option<&str>,
    theme: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    //INFO: Check if profile exists to determine insert vs update
    let existing = get_user_profile(connection)?;

    if existing.is_some() {
        //INFO: Update existing profile
        connection.execute(
            "UPDATE user_profile SET display_name = ?1, location = ?2, theme = ?3, updated_at = ?4 WHERE id = 1",
            params![display_name, location, theme, now],
        ).context("Failed to update user profile")?;
    } else {
        //INFO: Insert new profile
        connection.execute(
            "INSERT INTO user_profile (id, display_name, location, theme, created_at, updated_at) VALUES (1, ?1, ?2, ?3, ?4, ?4)",
            params![display_name, location, theme, now],
        ).context("Failed to insert user profile")?;
    }

    Ok(())
}

// ============================================================================
// Hotkey Queries
// ============================================================================

//INFO: Gets the current hotkey configuration
pub fn get_hotkey_config(connection: &Connection) -> Result<Option<HotkeyConfig>> {
    let result = connection
        .query_row(
            "SELECT modifier_keys, key, enabled FROM hotkey_config WHERE id = 1",
            [],
            |row| {
                let modifier_keys_json: String = row.get(0)?;
                let modifier_keys: Vec<String> =
                    serde_json::from_str(&modifier_keys_json).unwrap_or_default();
                Ok(HotkeyConfig {
                    modifier_keys,
                    key: row.get(1)?,
                    enabled: row.get::<_, i32>(2)? == 1,
                })
            },
        )
        .optional()
        .context("Failed to query hotkey config")?;

    Ok(result)
}

//INFO: Saves the hotkey configuration
pub fn save_hotkey_config(connection: &Connection, config: &HotkeyConfig) -> Result<()> {
    let modifier_keys_json = serde_json::to_string(&config.modifier_keys)
        .context("Failed to serialize modifier keys")?;

    connection.execute(
        "INSERT OR REPLACE INTO hotkey_config (id, modifier_keys, key, enabled) VALUES (1, ?1, ?2, ?3)",
        params![modifier_keys_json, config.key, config.enabled as i32],
    ).context("Failed to save hotkey config")?;

    Ok(())
}

// ============================================================================
// API Token Queries
// ============================================================================

//INFO: Saves an encrypted API token
pub fn save_api_token(
    connection: &Connection,
    provider: &str,
    encrypted_token: &str,
    token_type: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT OR REPLACE INTO api_tokens (provider, encrypted_token, token_type, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![provider, encrypted_token, token_type, now],
    ).context("Failed to save API token")?;
    Ok(())
}

//INFO: Gets an encrypted API token by provider name
pub fn get_api_token(connection: &Connection, provider: &str) -> Result<Option<String>> {
    let result: Option<String> = connection
        .query_row(
            "SELECT encrypted_token FROM api_tokens WHERE provider = ?1",
            params![provider],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query API token")?;

    Ok(result)
}

//INFO: Checks if an API token exists for a provider
#[allow(dead_code)]
pub fn has_api_token(connection: &Connection, provider: &str) -> Result<bool> {
    let result = get_api_token(connection, provider)?;
    Ok(result.is_some())
}

// ============================================================================
// Chat Message Queries
// ============================================================================

//INFO: Saves a chat message
pub fn save_chat_message(connection: &Connection, message: &ChatMessage) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO chat_messages (role, content, image_data, created_at, session_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![message.role, message.content, message.image_data, now, message.session_id],
    ).context("Failed to save chat message")?;

    Ok(connection.last_insert_rowid())
}

//INFO: Gets chat messages for a session
pub fn get_chat_messages(
    connection: &Connection,
    session_id: Option<&str>,
    limit: i32,
) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();

    //INFO: Build and execute query based on whether session_id is provided
    match session_id {
        Some(sid) => {
            let mut statement = connection.prepare(
                "SELECT id, role, content, image_data, created_at, session_id FROM chat_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ).context("Failed to prepare chat messages query")?;

            let rows = statement
                .query_map(params![sid, limit], |row| {
                    Ok(ChatMessage {
                        id: Some(row.get(0)?),
                        role: row.get(1)?,
                        content: row.get(2)?,
                        image_data: row.get(3)?,
                        created_at: row.get(4)?,
                        session_id: row.get(5)?,
                    })
                })
                .context("Failed to query chat messages")?;

            for row in rows {
                messages.push(row.context("Failed to parse chat message")?);
            }
        }
        None => {
            let mut statement = connection.prepare(
                "SELECT id, role, content, image_data, created_at, session_id FROM chat_messages ORDER BY created_at DESC LIMIT ?1"
            ).context("Failed to prepare chat messages query")?;

            let rows = statement
                .query_map(params![limit], |row| {
                    Ok(ChatMessage {
                        id: Some(row.get(0)?),
                        role: row.get(1)?,
                        content: row.get(2)?,
                        image_data: row.get(3)?,
                        created_at: row.get(4)?,
                        session_id: row.get(5)?,
                    })
                })
                .context("Failed to query chat messages")?;

            for row in rows {
                messages.push(row.context("Failed to parse chat message")?);
            }
        }
    };

    //INFO: Reverse to get chronological order
    messages.reverse();

    Ok(messages)
}

//INFO: Clears all chat messages
pub fn clear_chat_messages(connection: &Connection) -> Result<()> {
    connection
        .execute("DELETE FROM chat_messages", [])
        .context("Failed to clear chat messages")?;
    Ok(())
}

// ============================================================================
// Integration Queries
// ============================================================================

//INFO: Gets an integration by name
pub fn get_integration(connection: &Connection, name: &str) -> Result<Option<Integration>> {
    let result = connection
        .query_row(
            "SELECT name, enabled, config, last_sync, status FROM integrations WHERE name = ?1",
            params![name],
            |row| {
                Ok(Integration {
                    name: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? == 1,
                    config: row.get(2)?,
                    last_sync: row.get(3)?,
                    status: row.get(4)?,
                })
            },
        )
        .optional()
        .context("Failed to query integration")?;

    Ok(result)
}

//INFO: Saves or updates an integration
pub fn save_integration(connection: &Connection, integration: &Integration) -> Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO integrations (name, enabled, config, last_sync, status) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            integration.name,
            integration.enabled as i32,
            integration.config,
            integration.last_sync,
            integration.status
        ],
    ).context("Failed to save integration")?;
    Ok(())
}

//INFO: Gets all integrations
pub fn get_all_integrations(connection: &Connection) -> Result<Vec<Integration>> {
    let mut integrations = Vec::new();
    let mut statement = connection
        .prepare("SELECT name, enabled, config, last_sync, status FROM integrations")
        .context("Failed to prepare integrations query")?;

    let rows = statement
        .query_map([], |row| {
            Ok(Integration {
                name: row.get(0)?,
                enabled: row.get::<_, i32>(1)? == 1,
                config: row.get(2)?,
                last_sync: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .context("Failed to query integrations")?;

    for row in rows {
        integrations.push(row.context("Failed to parse integration")?);
    }

    Ok(integrations)
}

// ============================================================================
// Settings Queries
// ============================================================================

//INFO: Gets a setting by key
pub fn get_setting(connection: &Connection, key: &str) -> Result<Option<String>> {
    let result: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query setting")?;

    Ok(result)
}

//INFO: Saves a setting
pub fn save_setting(connection: &Connection, key: &str, value: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )
        .context("Failed to save setting")?;
    Ok(())
}

// ============================================================================
// Calendar Queries
// ============================================================================

//INFO: Saves calendar events (bulk insert/update)
#[allow(dead_code)]
pub fn save_calendar_events(connection: &Connection, events: &[CalendarEvent]) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    for event in events {
        connection.execute(
            "INSERT OR REPLACE INTO calendar_events (id, title, description, start_time, end_time, location, all_day, cached_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.id,
                event.title,
                event.description,
                event.start_time,
                event.end_time,
                event.location,
                event.all_day as i32,
                now
            ],
        ).context("Failed to save calendar event")?;
    }

    Ok(())
}

//INFO: Gets calendar events for a date range
pub fn get_calendar_events(
    connection: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<CalendarEvent>> {
    let mut events = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT id, title, description, start_time, end_time, location, all_day 
         FROM calendar_events 
         WHERE start_time >= ?1 AND start_time <= ?2 
         ORDER BY start_time ASC",
        )
        .context("Failed to prepare calendar events query")?;

    let rows = statement
        .query_map(params![start_date, end_date], |row| {
            Ok(CalendarEvent {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                location: row.get(5)?,
                all_day: row.get::<_, i32>(6)? == 1,
            })
        })
        .context("Failed to query calendar events")?;

    for row in rows {
        events.push(row.context("Failed to parse calendar event")?);
    }

    Ok(events)
}

//INFO: Clears all cached calendar events
#[allow(dead_code)]
pub fn clear_calendar_events(connection: &Connection) -> Result<()> {
    connection
        .execute("DELETE FROM calendar_events", [])
        .context("Failed to clear calendar events")?;
    Ok(())
}

// ============================================================================
// Briefing Queries
// ============================================================================

// INFO: Saves a new briefing summary
pub fn save_briefing_summary(
    connection: &Connection,
    content: &str,
    data_hash: &str,
    audio_data: Option<&[u8]>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO briefing_summaries (content, data_hash, audio_data, created_at) VALUES (?, ?, ?, ?)",
        params![content, data_hash, audio_data, now],
    )?;
    Ok(())
}

// INFO: Gets the latest briefing summary
pub fn get_latest_briefing_summary(connection: &Connection) -> Result<Option<BriefingSummary>> {
    connection.query_row(
        "SELECT id, content, data_hash, audio_data, created_at, is_final_of_day FROM briefing_summaries ORDER BY created_at DESC LIMIT 1",
        [],
        |row| Ok(BriefingSummary {
            id: row.get(0)?,
            content: row.get(1)?,
            data_hash: row.get(2)?,
            audio_data: row.get(3)?,
            created_at: row.get(4)?,
            is_final_of_day: row.get::<_, i32>(5)? != 0,
        })
    ).optional().context("Failed to get latest briefing summary")
}

// INFO: Gets the last briefing from before today for evolutionary context
pub fn get_yesterdays_final_briefing(connection: &Connection) -> Result<Option<BriefingSummary>> {
    // Search for the most recent summary created before today's start
    connection
        .query_row(
            "SELECT id, content, data_hash, audio_data, created_at, is_final_of_day 
         FROM briefing_summaries 
         WHERE created_at < date('now', 'start of day')
         ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok(BriefingSummary {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    data_hash: row.get(2)?,
                    audio_data: row.get(3)?,
                    created_at: row.get(4)?,
                    is_final_of_day: row.get::<_, i32>(5)? != 0,
                })
            },
        )
        .optional()
        .context("Failed to get historical briefing context")
}

// INFO: Gets all summaries from today for evolution context
pub fn get_todays_briefings(connection: &Connection) -> Result<Vec<BriefingSummary>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();

    let mut stmt = connection.prepare(
        "SELECT id, content, data_hash, audio_data, created_at, is_final_of_day 
         FROM briefing_summaries 
         WHERE created_at LIKE ? 
         ORDER BY created_at ASC",
    )?;

    let briefings = stmt
        .query_map([format!("{}%", today)], |row| {
            Ok(BriefingSummary {
                id: row.get(0)?,
                content: row.get(1)?,
                data_hash: row.get(2)?,
                audio_data: row.get(3)?,
                created_at: row.get(4)?,
                is_final_of_day: row.get::<_, i32>(5)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(briefings)
}

// INFO: Marks a briefing as final (e.g. at the end of the day)
pub fn mark_briefing_as_final(connection: &Connection, id: i32) -> Result<()> {
    connection.execute(
        "UPDATE briefing_summaries SET is_final_of_day = 1 WHERE id = ?",
        params![id],
    )?;
    Ok(())
}
// ============================================================================
// Notification Queries
// ============================================================================

// INFO: Checks if a notification has already been sent for an item
pub fn has_notified(connection: &Connection, external_id: &str, provider: &str) -> Result<bool> {
    let result: Option<i32> = connection
        .query_row(
            "SELECT 1 FROM notifications WHERE external_id = ?1 AND provider = ?2",
            params![external_id, provider],
            |_| Ok(1),
        )
        .optional()
        .context("Failed to check notification status")?;

    Ok(result.is_some())
}

// INFO: Records that a notification was sent
pub fn record_notification(
    connection: &Connection,
    external_id: &str,
    provider: &str,
    title: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO notifications (external_id, provider, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![external_id, provider, title, now],
    ).context("Failed to record notification")?;
    Ok(())
}

// INFO: Saves a clipboard item to history
pub fn save_clipboard_item(
    connection: &Connection,
    content: &str,
    content_type: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO clipboard_history (content, type, created_at) VALUES (?1, ?2, ?3)",
            params![content, content_type, now],
        )
        .context("Failed to save clipboard item")?;
    Ok(())
}

// INFO: Searches the clipboard history for a specific query
pub fn search_clipboard_history(
    connection: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    let mut stmt = connection.prepare(
        "SELECT content, created_at FROM clipboard_history 
         WHERE content LIKE ?1 
         ORDER BY created_at DESC 
         LIMIT ?2",
    )?;

    let pattern = format!("%{}%", query);
    let rows = stmt.query_map(params![pattern, limit], |row| {
        Ok(serde_json::json!({
            "content": row.get::<_, String>(0)?,
            "timestamp": row.get::<_, String>(1)?
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}
