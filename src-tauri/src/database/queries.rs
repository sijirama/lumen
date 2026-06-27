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
    #[serde(default = "default_snipper_modifiers")]
    pub snipper_modifier_keys: Vec<String>,
    #[serde(default = "default_snipper_key")]
    pub snipper_key: String,
    #[serde(default = "default_snipper_enabled")]
    pub snipper_enabled: bool,
}

fn default_snipper_modifiers() -> Vec<String> {
    vec!["Super".to_string(), "Shift".to_string()]
}
fn default_snipper_key() -> String {
    "S".to_string()
}
fn default_snipper_enabled() -> bool {
    true
}

//INFO: Citation data structure for web search sources
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Citation {
    pub title: String,
    pub url: String,
}

//INFO: One tool call's persisted trace — what was invoked, with what args,
//      what came back, and how long it took. Surfaces in the chat UI as a
//      collapsible card so the user can audit what Lumen actually did.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolInvocation {
    pub name: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
    pub duration_ms: u64,
    pub started_at: String, // RFC3339
    pub succeeded: bool,
}

//INFO: Chat message data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: Option<i64>,
    pub role: String,
    pub content: String,
    pub image_data: Option<String>,
    pub citations: Option<Vec<Citation>>,
    pub tool_invocations: Option<Vec<ToolInvocation>>,
    pub created_at: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reminder {
    pub id: i32,
    pub content: String,
    pub due_at: Option<String>,
    pub completed: bool,
    pub created_at: String,
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
            "SELECT modifier_keys, key, enabled, snipper_modifier_keys, snipper_key, snipper_enabled FROM hotkey_config WHERE id = 1",
            [],
            |row| {
                let modifier_keys_json: String = row.get(0)?;
                let modifier_keys: Vec<String> =
                    serde_json::from_str(&modifier_keys_json).unwrap_or_default();
                    
                let snip_mod_json: Option<String> = row.get(3).unwrap_or(None);
                let snipper_modifier_keys: Vec<String> = match snip_mod_json {
                    Some(json) => serde_json::from_str(&json).unwrap_or_else(|_| default_snipper_modifiers()),
                    None => default_snipper_modifiers()
                };

                let snipper_key: String = row.get(4).unwrap_or_else(|_| default_snipper_key());
                let snipper_enabled: bool = row.get::<_, i32>(5).unwrap_or(1) == 1;

                Ok(HotkeyConfig {
                    modifier_keys,
                    key: row.get(1)?,
                    enabled: row.get::<_, i32>(2)? == 1,
                    snipper_modifier_keys,
                    snipper_key,
                    snipper_enabled,
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
    
    let snipper_modifiers_json = serde_json::to_string(&config.snipper_modifier_keys)
        .context("Failed to serialize snipper modifier keys")?;

    connection.execute(
        "INSERT OR REPLACE INTO hotkey_config (id, modifier_keys, key, enabled, snipper_modifier_keys, snipper_key, snipper_enabled) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        params![modifier_keys_json, config.key, config.enabled as i32, snipper_modifiers_json, config.snipper_key, config.snipper_enabled as i32],
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
pub fn has_api_token(connection: &Connection, provider: &str) -> Result<bool> {
    let result = get_api_token(connection, provider)?;
    Ok(result.is_some())
}

// ============================================================================
// Chat Message Queries
// ============================================================================

//INFO: Per-result size cap when persisting tool invocations. Keeps the chat_messages
//      row from bloating if a tool returns a huge payload (e.g. read_file on a big note).
//      The full result is still available to the model in its message history during the
//      turn — this only affects what's persisted for the user to inspect later.
const MAX_PERSISTED_RESULT_BYTES: usize = 4096;

fn truncate_tool_result_for_storage(name: &str, value: &serde_json::Value) -> serde_json::Value {
    // take_screenshot intentionally bypasses truncation — the b64 image_data
    // is what powers the inline thumbnail in the tool trace.
    if name == "take_screenshot" {
        return value.clone();
    }
    match serde_json::to_string(value) {
        Ok(s) if s.len() > MAX_PERSISTED_RESULT_BYTES => serde_json::json!({
            "_truncated": true,
            "original_size_bytes": s.len(),
            "preview": s.chars().take(MAX_PERSISTED_RESULT_BYTES).collect::<String>()
        }),
        _ => value.clone(),
    }
}

//INFO: Saves a chat message
pub fn save_chat_message(connection: &Connection, message: &ChatMessage) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let citations_json = message.citations.as_ref().and_then(|c| serde_json::to_string(c).ok());

    // Truncate oversized tool results before persistence so the DB row stays sane.
    let tool_invocations_json = message.tool_invocations.as_ref().and_then(|invocations| {
        let trimmed: Vec<ToolInvocation> = invocations
            .iter()
            .map(|inv| ToolInvocation {
                name: inv.name.clone(),
                args: inv.args.clone(),
                result: truncate_tool_result_for_storage(&inv.name, &inv.result),
                duration_ms: inv.duration_ms,
                started_at: inv.started_at.clone(),
                succeeded: inv.succeeded,
            })
            .collect();
        serde_json::to_string(&trimmed).ok()
    });

    connection.execute(
        "INSERT INTO chat_messages (role, content, image_data, citations, tool_invocations, created_at, session_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![message.role, message.content, message.image_data, citations_json, tool_invocations_json, now, message.session_id],
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
                "SELECT id, role, content, image_data, created_at, session_id, citations, tool_invocations FROM chat_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ).context("Failed to prepare chat messages query")?;

            let rows = statement
                .query_map(params![sid, limit], |row| {
                    let citations_json: Option<String> = row.get(6)?;
                    let citations = citations_json.and_then(|s| serde_json::from_str(&s).ok());
                    let tool_invocations_json: Option<String> = row.get(7)?;
                    let tool_invocations = tool_invocations_json.and_then(|s| serde_json::from_str(&s).ok());

                    Ok(ChatMessage {
                        id: Some(row.get(0)?),
                        role: row.get(1)?,
                        content: row.get(2)?,
                        image_data: row.get(3)?,
                        created_at: row.get(4)?,
                        session_id: row.get(5)?,
                        citations,
                        tool_invocations,
                    })
                })
                .context("Failed to query chat messages")?;

            for row in rows {
                messages.push(row.context("Failed to parse chat message")?);
            }
        }
        None => {
            let mut statement = connection.prepare(
                "SELECT id, role, content, image_data, created_at, session_id, citations, tool_invocations FROM chat_messages ORDER BY created_at DESC LIMIT ?1"
            ).context("Failed to prepare chat messages query")?;

            let rows = statement
                .query_map(params![limit], |row| {
                    let citations_json: Option<String> = row.get(6)?;
                    let citations = citations_json.and_then(|s| serde_json::from_str(&s).ok());
                    let tool_invocations_json: Option<String> = row.get(7)?;
                    let tool_invocations = tool_invocations_json.and_then(|s| serde_json::from_str(&s).ok());

                    Ok(ChatMessage {
                        id: Some(row.get(0)?),
                        role: row.get(1)?,
                        content: row.get(2)?,
                        image_data: row.get(3)?,
                        created_at: row.get(4)?,
                        session_id: row.get(5)?,
                        citations,
                        tool_invocations,
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

//INFO: Count total chat messages (used for mod-trigger memory extraction)
pub fn count_chat_messages(connection: &Connection) -> Result<i64> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM chat_messages", [], |row| row.get(0))
        .context("Failed to count chat messages")?;
    Ok(count)
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

//INFO: Count total clipboard items (used for mod-trigger memory extraction)
pub fn count_clipboard_items(connection: &Connection) -> Result<i64> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM clipboard_history", [], |row| row.get(0))
        .context("Failed to count clipboard items")?;
    Ok(count)
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

// INFO: Represents an item from the clipboard history
#[derive(Debug, Serialize, Deserialize)]
pub struct ClipboardHistoryItem {
    pub content: String,
    pub content_type: String,
    pub created_at: String,
}

// INFO: Gets the most recent clipboard items
pub fn get_recent_clipboard_items(
    connection: &Connection,
    limit: u32,
) -> Result<Vec<ClipboardHistoryItem>> {
    let mut stmt = connection.prepare(
        "SELECT content, type, created_at FROM clipboard_history 
         ORDER BY created_at DESC 
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        Ok(ClipboardHistoryItem {
            content: row.get(0)?,
            content_type: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// INFO: Searches the clipboard history with optional filters
pub fn search_clipboard_history_filtered(
    connection: &Connection,
    query: &str,
    after: Option<&str>,
    before: Option<&str>,
    content_type: Option<&str>,
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    let mut sql = "SELECT content, created_at, type FROM clipboard_history WHERE content LIKE ?1".to_string();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("%{}%", query))];
    let mut param_idx = 2;

    if let Some(a) = after {
        sql.push_str(&format!(" AND created_at >= ?{}", param_idx));
        params.push(Box::new(a.to_string()));
        param_idx += 1;
    }
    if let Some(b) = before {
        sql.push_str(&format!(" AND created_at <= ?{}", param_idx));
        params.push(Box::new(b.to_string()));
        param_idx += 1;
    }
    if let Some(ct) = content_type {
        sql.push_str(&format!(" AND type = ?{}", param_idx));
        params.push(Box::new(ct.to_string()));
        param_idx += 1;
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT ?");
    sql.push_str(&param_idx.to_string());
    params.push(Box::new(limit));

    let mut stmt = connection.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(serde_json::json!({
            "content": row.get::<_, String>(0)?,
            "timestamp": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn search_clipboard_history(
    connection: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    search_clipboard_history_filtered(connection, query, None, None, None, limit)
}

// ============================================================================
// Web Search Cache Queries
// ============================================================================

//INFO: Gets a cached search result if it exists
pub fn get_web_search_cache(connection: &Connection, query_hash: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT results FROM web_search_cache WHERE query_hash = ?",
            [query_hash],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to get web search cache")
}

//INFO: Saves a search result to the cache
pub fn save_web_search_cache(
    connection: &Connection,
    query_hash: &str,
    query: &str,
    results: &str,
) -> Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO web_search_cache (query_hash, query, results, cached_at) VALUES (?, ?, ?, ?)",
        params![query_hash, query, results, Utc::now().to_rfc3339()],
    ).context("Failed to save web search cache")?;
    Ok(())
}

// ============================================================================
// Reminder Queries
// ============================================================================

//INFO: Saves a new reminder to the database
pub fn save_reminder(
    connection: &Connection,
    content: &str,
    due_at: Option<&str>,
) -> Result<i32> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO reminders (content, due_at, created_at) VALUES (?, ?, ?)",
        params![content, due_at, now],
    ).context("Failed to save reminder")?;
    Ok(connection.last_insert_rowid() as i32)
}

//INFO: Gets all active (incomplete) reminders
pub fn get_active_reminders(connection: &Connection) -> Result<Vec<Reminder>> {
    let mut stmt = connection.prepare(
        "SELECT id, content, due_at, completed, created_at FROM reminders WHERE completed = 0 ORDER BY due_at ASC"
    )?;
    
    let rows = stmt.query_map([], |row| {
        Ok(Reminder {
            id: row.get(0)?,
            content: row.get(1)?,
            due_at: row.get(2)?,
            completed: row.get::<_, i32>(3)? == 1,
            created_at: row.get(4)?,
        })
    })?;

    let mut reminders = Vec::new();
    for row in rows {
        reminders.push(row?);
    }
    Ok(reminders)
}

//INFO: Gets reminders for a specific date range (ISO format)
pub fn get_reminders_for_range(
    connection: &Connection,
    start_iso: &str,
    end_iso: &str,
) -> Result<Vec<Reminder>> {
    let mut stmt = connection.prepare(
        "SELECT id, content, due_at, completed, created_at 
         FROM reminders 
         WHERE due_at >= ?1 AND due_at <= ?2"
    )?;
    
    let rows = stmt.query_map(params![start_iso, end_iso], |row| {
        Ok(Reminder {
            id: row.get(0)?,
            content: row.get(1)?,
            due_at: row.get(2)?,
            completed: row.get::<_, i32>(3)? == 1,
            created_at: row.get(4)?,
        })
    })?;

    let mut reminders = Vec::new();
    for row in rows {
        reminders.push(row?);
    }
    Ok(reminders)
}

//INFO: Deletes a reminder by ID
pub fn delete_reminder(connection: &Connection, id: i32) -> Result<()> {
    connection.execute("DELETE FROM reminders WHERE id = ?", params![id])
        .context("Failed to delete reminder")?;
    Ok(())
}

//INFO: Toggles the completion status of a reminder
pub fn toggle_reminder_completion(connection: &Connection, id: i32, completed: bool) -> Result<()> {
    let val = if completed { 1 } else { 0 };
    connection.execute("UPDATE reminders SET completed = ? WHERE id = ?", params![val, id])
        .context("Failed to update reminder completion")?;
    Ok(())
}

// ============================================================================
// Calendar Reminder Sync Queries
// ============================================================================

//INFO: Check if a calendar-sourced reminder already exists for a given event id
pub fn calendar_reminder_exists(connection: &Connection, event_id: &str) -> bool {
    let search_pattern = format!("%[cal:{}]%", event_id);
    connection
        .query_row(
            "SELECT COUNT(*) FROM reminders WHERE content LIKE ?1 AND completed = 0",
            params![search_pattern],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(0) > 0
}

//INFO: Create a reminder (used by the scheduler for calendar-aware reminders)
pub fn create_reminder(connection: &Connection, content: &str, due_at: Option<&str>) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO reminders (content, due_at, completed, created_at) VALUES (?1, ?2, 0, ?3)",
        params![content, due_at, now],
    ).context("Failed to create reminder")?;
    Ok(())
}


// ============================================================================
// Session Queries
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatSession {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub created_at: String,
    pub message_count: i64,
}

pub fn create_session(connection: &Connection) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO chat_sessions (id, created_at) VALUES (?1, ?2)",
            params![id, now],
        )
        .context("Failed to create chat session")?;
    Ok(id)
}

pub fn get_sessions(connection: &Connection) -> Result<Vec<ChatSession>> {
    let mut stmt = connection
        .prepare(
            "SELECT s.id, s.title, s.summary, s.created_at, COUNT(m.id) as message_count
             FROM chat_sessions s
             LEFT JOIN chat_messages m ON m.session_id = s.id
             GROUP BY s.id
             ORDER BY s.created_at DESC
             LIMIT 50",
        )
        .context("Failed to prepare get_sessions query")?;

    let sessions = stmt
        .query_map([], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                created_at: row.get(3)?,
                message_count: row.get(4)?,
            })
        })
        .context("Failed to query sessions")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sessions)
}

/// Sets the session title only if it has never been set before (first message wins).
pub fn update_session_title(connection: &Connection, session_id: &str, title: &str) -> Result<()> {
    connection
        .execute(
            "UPDATE chat_sessions SET title=?1 WHERE id=?2 AND title IS NULL",
            params![title, session_id],
        )
        .context("Failed to update session title")?;
    Ok(())
}

pub fn update_session_summary(connection: &Connection, session_id: &str, summary: &str) -> Result<()> {
    connection
        .execute(
            "UPDATE chat_sessions SET summary=?1 WHERE id=?2",
            params![summary, session_id],
        )
        .context("Failed to update session summary")?;
    Ok(())
}

