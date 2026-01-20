//INFO: Database connection management for Lumen
//NOTE: Uses SQLite with a single portable file stored in user's config directory

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;

//INFO: Thread-safe database wrapper
//NOTE: Wrapped in Mutex for safe concurrent access from multiple Tauri commands
pub struct Database {
    pub connection: Mutex<Connection>,
    pub database_path: PathBuf,
}

impl Database {
    //INFO: Creates a new database connection
    //NOTE: Automatically creates the database file and parent directories if they don't exist
    pub fn new() -> Result<Self> {
        //INFO: Get the platform-appropriate config directory for storing the database
        let config_directory = get_config_directory()?;

        //INFO: Ensure the config directory exists
        std::fs::create_dir_all(&config_directory).context("Failed to create config directory")?;

        //INFO: Construct the full path to the database file
        let database_path = config_directory.join("lumen.db");

        //INFO: Open or create the SQLite database connection
        let connection =
            Connection::open(&database_path).context("Failed to open database connection")?;

        //INFO: Enable foreign key support for referential integrity
        connection
            .execute("PRAGMA foreign_keys = ON", [])
            .context("Failed to enable foreign keys")?;

        Ok(Self {
            connection: Mutex::new(connection),
            database_path,
        })
    }

    //INFO: Returns the path to the database file
    //NOTE: Useful for export/import functionality
    pub fn get_database_path(&self) -> &PathBuf {
        &self.database_path
    }
}

//INFO: Gets the platform-appropriate configuration directory for Lumen
//NOTE: Linux: ~/.config/lumen, macOS: ~/Library/Application Support/lumen, Windows: %APPDATA%\lumen
fn get_config_directory() -> Result<PathBuf> {
    let config_dir =
        dirs::config_dir().context("Failed to determine config directory for this platform")?;

    Ok(config_dir.join("lumen"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        //INFO: Test that database can be created successfully
        let database = Database::new();
        assert!(database.is_ok());
    }
}
