//INFO: Tool definitions and handlers for Gemini Function Calling
//NOTE: Implements file system operations for Obsidian integration

use crate::gemini::client::{GeminiFunctionDeclaration, GeminiTool};
use serde_json::json;
use std::fs;
use walkdir::WalkDir;

//INFO: Get all available tool declarations for Gemini
pub fn get_tool_declarations() -> Vec<GeminiTool> {
    vec![GeminiTool {
        function_declarations: vec![
            GeminiFunctionDeclaration {
                name: "read_file".to_string(),
                description: "Reads the content of a file at the specified path.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The absolute path to the file to read."
                        }
                    },
                    "required": ["path"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "write_file".to_string(),
                description:
                    "Writes content to a file at the specified path. Overwrites if it exists."
                        .to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The absolute path to the file."
                        },
                        "content": {
                            "type": "string",
                            "description": "The content to write to the file."
                        }
                    },
                    "required": ["path", "content"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "list_files".to_string(),
                description: "Lists files in a directory.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The absolute path to the directory."
                        }
                    },
                    "required": ["path"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "search_notes".to_string(),
                description: "Searches for a keyword inside all markdown files in a directory."
                    .to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The absolute path to the directory (usually the vault root)."
                        },
                        "query": {
                            "type": "string",
                            "description": "The keyword to search for."
                        }
                    },
                    "required": ["path", "query"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "get_obsidian_vault_info".to_string(),
                description:
                    "Gets information about the configured Obsidian vault, including its root path."
                        .to_string(),
                parameters: None,
            },
            GeminiFunctionDeclaration {
                name: "add_reminder".to_string(),
                description: "Adds a reminder for the user.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The reminder text."
                        },
                        "due_at": {
                            "type": "string",
                            "description": "When the reminder is due (optional, e.g. '2026-01-20T10:00:00Z')."
                        }
                    },
                    "required": ["content"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "list_reminders".to_string(),
                description: "Lists all active reminders.".to_string(),
                parameters: None,
            },
            GeminiFunctionDeclaration {
                name: "search_web".to_string(),
                description: "Searches the web for a query (simulated).".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query."
                        }
                    },
                    "required": ["query"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "get_weather".to_string(),
                description: "Gets the current weather for a location (simulated).".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city or location."
                        }
                    },
                    "required": ["location"]
                })),
            },
        ],
    }]
}

//INFO: Execute a synchronous tool call and return the result as JSON
pub fn execute_tool_sync(
    name: &str,
    args: &serde_json::Value,
    obsidian_config: Option<&serde_json::Value>,
    db_connection: &rusqlite::Connection,
) -> serde_json::Value {
    match name {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match fs::read_to_string(path) {
                Ok(content) => json!({ "content": content }),
                Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
            }
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match fs::write(path, content) {
                Ok(_) => json!({ "status": "success" }),
                Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
            }
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            match fs::read_dir(path) {
                Ok(entries) => {
                    let files: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            if e.path().is_dir() {
                                format!("{}/", name)
                            } else {
                                name
                            }
                        })
                        .collect();
                    json!({ "entries": files, "current_path": path })
                }
                Err(e) => json!({ "error": format!("Failed to list directory: {}", e) }),
            }
        }
        "search_notes" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            if path.is_empty() || query.is_empty() {
                return json!({ "error": "Path and query are required for searching." });
            }

            let mut results = Vec::new();
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file()
                    && entry.path().extension().map_or(false, |ext| ext == "md")
                {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if content.to_lowercase().contains(&query) {
                            results.push(entry.path().to_string_lossy().into_owned());
                        }
                    }
                }
                if results.len() >= 10 {
                    break;
                } // Limit results
            }
            json!({ "matches": results })
        }
        "get_obsidian_vault_info" => {
            if let Some(config) = obsidian_config {
                json!({
                    "vault_path": config.get("vault_path"),
                    "daily_notes_folder": config.get("daily_notes_path").and_then(|v| v.as_str()).unwrap_or(""),
                    "daily_notes_format": config.get("daily_notes_format").and_then(|v| v.as_str()).unwrap_or("YYYY-MM-DD"),
                    "status": "configured"
                })
            } else {
                json!({ "error": "Obsidian vault not configured in settings." })
            }
        }
        "add_reminder" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let due_at = args.get("due_at").and_then(|v| v.as_str());
            let created_at = chrono::Utc::now().to_rfc3339();

            match db_connection.execute(
                "INSERT INTO reminders (content, due_at, created_at) VALUES (?, ?, ?)",
                rusqlite::params![content, due_at, created_at],
            ) {
                Ok(_) => json!({ "status": "success", "message": "Reminder added." }),
                Err(e) => json!({ "error": format!("Failed to add reminder: {}", e) }),
            }
        }
        "list_reminders" => {
            let mut stmt = match db_connection
                .prepare("SELECT id, content, due_at, completed FROM reminders WHERE completed = 0")
            {
                Ok(s) => s,
                Err(e) => return json!({ "error": e.to_string() }),
            };

            let reminders: Vec<_> = stmt
                .query_map([], |row| {
                    Ok(json!({
                        "id": row.get::<_, i32>(0)?,
                        "content": row.get::<_, String>(1)?,
                        "due_at": row.get::<_, Option<String>>(2)?,
                        "completed": row.get::<_, i32>(3)? == 1
                    }))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            json!({ "reminders": reminders })
        }
        "search_web" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            // Simulate a search result for now
            json!({
                "results": [
                    { "title": format!("Information about {}", query), "snippet": "This is a simulated search result from the web." },
                    { "title": "Lumen AI Assistant", "snippet": "Lumen is a desktop AI assistant designed for productivity." }
                ]
            })
        }
        _ => json!({ "error": format!("Unknown synchronous tool: {}", name) }),
    }
}

//INFO: Execute an asynchronous tool call and return the result as JSON
pub async fn execute_tool_async(name: &str, args: &serde_json::Value) -> serde_json::Value {
    match name {
        "get_weather" => {
            let location = args
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("Lagos");
            fetch_weather(location).await
        }
        _ => json!({ "error": format!("Unknown asynchronous tool: {}", name) }),
    }
}

//INFO: Standalone weather fetch for internal use
pub async fn fetch_weather(location: &str) -> serde_json::Value {
    let url = format!("https://wttr.in/{}?format=j1", location);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({ "error": format!("Failed to create client: {}", e) }),
    };

    match client.get(&url).send().await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(current) = data
                    .get("current_condition")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                {
                    let temp = current
                        .get("temp_C")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let desc = current
                        .get("weatherDesc")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let humidity = current
                        .get("humidity")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    json!({
                        "location": location,
                        "temperature_c": temp,
                        "condition": desc,
                        "humidity": format!("{}%", humidity),
                        "source": "wttr.in"
                    })
                } else {
                    json!({ "error": "Could not parse weather data." })
                }
            }
            Err(e) => json!({ "error": format!("Failed to parse weather JSON: {}", e) }),
        },
        Err(e) => json!({ "error": format!("Failed to fetch weather: {}", e) }),
    }
}
