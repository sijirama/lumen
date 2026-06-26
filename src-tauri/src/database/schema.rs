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
            snipper_modifier_keys TEXT DEFAULT '[\"Super\",\"Shift\"]',
            snipper_key TEXT DEFAULT 'S',
            snipper_enabled INTEGER DEFAULT 1,
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
            citations TEXT,
            tool_invocations TEXT,
            created_at TEXT NOT NULL,
            session_id TEXT
        )",
            [],
        )
        .context("Failed to create chat_messages table")?;

    //INFO: Migrations — add columns that older databases don't have.
    {
        let mut statement = connection.prepare("PRAGMA table_info(chat_messages)")?;
        let columns: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|c| c.ok())
            .collect();

        if !columns.iter().any(|n| n == "citations") {
            crate::applog!("DEBUG: 🛠️ Migrating chat_messages table (adding citations column)");
            connection.execute("ALTER TABLE chat_messages ADD COLUMN citations TEXT", [])
                .context("Failed to migrate chat_messages table (adding citations column)")?;
        }
        if !columns.iter().any(|n| n == "tool_invocations") {
            crate::applog!("DEBUG: 🛠️ Migrating chat_messages table (adding tool_invocations column)");
            connection.execute("ALTER TABLE chat_messages ADD COLUMN tool_invocations TEXT", [])
                .context("Failed to migrate chat_messages table (adding tool_invocations column)")?;
        }
    }

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

    //INFO: Create clipboard_history table
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS clipboard_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            type TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create clipboard_history table")?;

    //INFO: Create memories table - stores observations, reflections, entities, preferences, and daily summaries
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL CHECK (type IN ('observation', 'reflection', 'entity', 'preference', 'daily_summary')),
            content TEXT NOT NULL,
            importance REAL NOT NULL DEFAULT 5.0,
            created_at TEXT NOT NULL,
            last_accessed TEXT NOT NULL,
            access_count INTEGER NOT NULL DEFAULT 0
        )",
            [],
        )
        .context("Failed to create memories table")?;

    //INFO: Create memory_embeddings virtual table - sqlite-vec for semantic vector search (768-dim for Gemini text-embedding-004)
    connection
        .execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_embeddings USING vec0(
            id TEXT PRIMARY KEY,
            embedding float[768]
        )",
            [],
        )
        .context("Failed to create memory_embeddings virtual table")?;

    //INFO: Create web_search_cache table - stores search results to save API credits
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS web_search_cache (
            query_hash TEXT PRIMARY KEY,
            query TEXT NOT NULL,
            results TEXT NOT NULL,
            cached_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create web_search_cache table")?;

    //INFO: Create lumen_tasks table — for queued background tasks Lumen wants to execute
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS lumen_tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            task_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'done', 'failed')),
            created_at TEXT NOT NULL,
            completed_at TEXT
        )",
            [],
        )
        .context("Failed to create lumen_tasks table")?;

    //INFO: Create chat_sessions table — tracks conversation sessions
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS chat_sessions (
            id TEXT PRIMARY KEY,
            title TEXT,
            summary TEXT,
            created_at TEXT NOT NULL
        )",
            [],
        )
        .context("Failed to create chat_sessions table")?;

    //INFO: Migration — add session_id column to chat_messages if missing
    {
        let mut statement = connection.prepare("PRAGMA table_info(chat_messages)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        let mut has_session_id = false;
        for col in columns {
            if let Ok(name) = col {
                if name == "session_id" {
                    has_session_id = true;
                    break;
                }
            }
        }
        if !has_session_id {
            connection.execute("ALTER TABLE chat_messages ADD COLUMN session_id TEXT", [])
                .context("Failed to migrate chat_messages table (adding session_id column)")?;
        }
    }

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
