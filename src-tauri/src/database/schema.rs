//INFO: Database schema definitions and initialization for Lumen
//NOTE: All tables are created here on first run

use anyhow::{Context, Result};
use rusqlite::Connection;

//INFO: Initializes all database tables if they don't exist
//NOTE: Called on application startup to ensure schema is ready
pub fn initialize_database(connection: &Connection) -> Result<()> {
    //INFO: Create user_profile table - stores the user's display name and location
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS user_profile (
            id INTEGER PRIMARY KEY DEFAULT 1,
            display_name TEXT NOT NULL,
            location TEXT,
            theme TEXT NOT NULL DEFAULT 'dark',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK (id = 1)
        )",
            [],
        )
        .context("Failed to create user_profile table")?;

    // Migration: Add location column if it doesn't exist
    let mut stmt = connection.prepare("PRAGMA table_info(user_profile)")?;
    let mut has_location = false;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "location" {
            has_location = true;
            break;
        }
    }

    if !has_location {
        connection.execute("ALTER TABLE user_profile ADD COLUMN location TEXT", [])?;
    }

    //INFO: Create settings table - key-value store for app settings
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create settings table")?;

    //INFO: Create hotkey_config table - stores the user's preferred hotkey
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS hotkey_config (
            id INTEGER PRIMARY KEY DEFAULT 1,
            modifier_keys TEXT NOT NULL,
            key TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            CHECK (id = 1)
        )",
            [],
        )
        .context("Failed to create hotkey_config table")?;

    //INFO: Create api_tokens table - stores encrypted API keys and OAuth tokens
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS api_tokens (
            provider TEXT PRIMARY KEY,
            encrypted_token TEXT NOT NULL,
            token_type TEXT NOT NULL,
            expires_at TEXT,
            updated_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create api_tokens table")?;

    //INFO: Create chat_messages table - stores conversation history
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS chat_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            image_data TEXT,
            created_at TEXT NOT NULL,
            session_id TEXT
        )",
            [],
        )
        .context("Failed to create chat_messages table")?;

    // Migration: Add image_data column if it doesn't exist
    let mut stmt = connection.prepare("PRAGMA table_info(chat_messages)")?;
    let mut has_image_data = false;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "image_data" {
            has_image_data = true;
            break;
        }
    }
    if !has_image_data {
        connection.execute("ALTER TABLE chat_messages ADD COLUMN image_data TEXT", [])?;
    }

    //INFO: Create calendar_events table - caches calendar events for offline access
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS calendar_events (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            location TEXT,
            all_day INTEGER NOT NULL DEFAULT 0,
            cached_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create calendar_events table")?;

    //INFO: Create integrations table - tracks integration status and config
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS integrations (
            name TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 0,
            config TEXT,
            last_sync TEXT,
            status TEXT NOT NULL DEFAULT 'disconnected'
        )",
            [],
        )
        .context("Failed to create integrations table")?;

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS setup_status (
            id INTEGER PRIMARY KEY DEFAULT 1,
            completed INTEGER NOT NULL DEFAULT 0,
            completed_at TEXT,
            CHECK (id = 1)
        )",
            [],
        )
        .context("Failed to create setup_status table")?;

    //INFO: Create reminders table
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS reminders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            due_at TEXT,
            completed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create reminders table")?;

    //INFO: Create web_cache table for fun tools
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS web_cache (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            expires_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create web_cache table")?;

    //INFO: Create briefing_summaries table for the dashboard
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS briefing_summaries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            data_hash TEXT NOT NULL,
            audio_data BLOB,
            created_at TEXT NOT NULL,
            is_final_of_day INTEGER NOT NULL DEFAULT 0
        )",
            [],
        )
        .context("Failed to create briefing_summaries table")?;

    // Migration: Add audio_data column if it doesn't exist
    let mut stmt = connection.prepare("PRAGMA table_info(briefing_summaries)")?;
    let mut has_audio = false;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "audio_data" {
            has_audio = true;
            break;
        }
    }
    if !has_audio {
        connection.execute(
            "ALTER TABLE briefing_summaries ADD COLUMN audio_data BLOB",
            [],
        )?;
    }

    //INFO: Create notifications table to track proactive pings
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS notifications (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            external_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(external_id, provider)
        )",
            [],
        )
        .context("Failed to create notifications table")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_initialization() {
        //INFO: Test that all tables can be created
        let connection = Connection::open_in_memory().unwrap();
        let result = initialize_database(&connection);
        assert!(result.is_ok());
    }
}
