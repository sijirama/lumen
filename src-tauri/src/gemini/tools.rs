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
                description: "Reads the content of a local file (e.g., an Obsidian note or daily task list).".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The absolute path to the local file."
                        }
                    },
                    "required": ["path"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "write_file".to_string(),
                description:
                    "Writes content to a local file. Use this for ticking tasks in daily notes OR updating vault content. Overwrites if it exists."
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
                            "description": "The content to write to the local file."
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
                description: "Gets the current weather for a location.".to_string(),
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
            GeminiFunctionDeclaration {
                name: "get_google_calendar_events".to_string(),
                description: "Lists Google Calendar events for a specific time range.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "time_min": {
                            "type": "string",
                            "description": "Start time in RFC3339 format (e.g. '2026-01-20T00:00:00Z')."
                        },
                        "time_max": {
                            "type": "string",
                            "description": "End time in RFC3339 format."
                        }
                    },
                    "required": ["time_min", "time_max"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "get_unread_emails".to_string(),
                description: "Lists recent emails from Gmail. Can filter by query (e.g. 'newer_than:1d', 'after:2026/01/20', 'from:person@example.com').".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of emails to fetch (default 5)."
                        },
                        "query": {
                            "type": "string",
                            "description": "Gmail search query. For today's emails use 'newer_than:1d'. Default is 'is:unread inbox'."
                        }
                    }
                })),
            },
            GeminiFunctionDeclaration {
                name: "send_email".to_string(),
                description: "Sends an email using Gmail.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "to": {
                            "type": "string",
                            "description": "Recipient email address."
                        },
                        "subject": {
                            "type": "string",
                            "description": "Email subject."
                        },
                        "body": {
                            "type": "string",
                            "description": "Email body content."
                        }
                    },
                    "required": ["to", "subject", "body"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "create_calendar_event".to_string(),
                description: "Creates a new event in the user's primary Google Calendar. IMPORTANT: Use the current year and the user's timezone offset from the 'ISO' time provided in CONTEXT (e.g. '2026-01-20T14:00:00+01:00')."
                    .to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": "Event title."
                        },
                        "description": {
                            "type": "string",
                            "description": "Event description."
                        },
                        "start_time": {
                            "type": "string",
                            "description": "Start time in RFC3339 format with offset (e.g. '2026-01-20T14:00:00+01:00')."
                        },
                        "end_time": {
                            "type": "string",
                            "description": "End time in RFC3339 format with offset."
                        },
                        "location": {
                            "type": "string",
                            "description": "Physical or virtual location."
                        }
                    },
                    "required": ["summary", "start_time", "end_time"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "list_google_tasks".to_string(),
                description: "Lists pending tasks from the user's default Google Tasks list (Official cloud-stored items). DO NOT use this for checking local Obsidian daily notes or Markdown tasks."
                    .to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of tasks to fetch (default 10)."
                        }
                    }
                })),
            },
            GeminiFunctionDeclaration {
                name: "create_google_task".to_string(),
                description: "Creates a new official cloud-stored task in Google Tasks. DO NOT use this for updating local Obsidian files. IMPORTANT: For due dates, use the current year and offset from the 'ISO' time in CONTEXT."
                    .to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Task title."
                        },
                        "notes": {
                            "type": "string",
                            "description": "Task notes/description."
                        },
                        "due": {
                            "type": "string",
                            "description": "Due date in RFC3339 format with offset (e.g. '2026-01-20T23:59:59+01:00')."
                        }
                    },
                    "required": ["title"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "grep_file".to_string(),
                description: "Searches for a pattern in a file and returns matching lines with line numbers.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file." },
                        "pattern": { "type": "string", "description": "The string to search for (case-insensitive)." }
                    },
                    "required": ["path", "pattern"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "edit_file_line".to_string(),
                description: "Replaces a specific line in a file by line number (1-indexed).".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file." },
                        "line_number": { "type": "integer", "description": "The 1-based line number to replace." },
                        "new_content": { "type": "string", "description": "The new content for that line." }
                    },
                    "required": ["path", "line_number", "new_content"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "insert_at_line".to_string(),
                description: "Inserts a new line at a specific line number (1-indexed). Everything else shifts down.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file." },
                        "line_number": { "type": "integer", "description": "The 1-based line number to insert at." },
                        "content": { "type": "string", "description": "The content to insert." }
                    },
                    "required": ["path", "line_number", "content"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "delete_file_line".to_string(),
                description: "Deletes a specific line from a file by line number (1-indexed).".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file." },
                        "line_number": { "type": "integer", "description": "The 1-based line number to delete." }
                    },
                    "required": ["path", "line_number"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "read_file_lines".to_string(),
                description: "Reads a specific range of lines from a file (1-indexed). Use this to verify context before editing.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file." },
                        "start_line": { "type": "integer", "description": "The first line to read." },
                        "end_line": { "type": "integer", "description": "The last line to read." }
                    },
                    "required": ["path", "start_line", "end_line"]
                })),
            },
            GeminiFunctionDeclaration {
                name: "take_screenshot".to_string(),
                description: "Captures a screenshot of the user's primary screen so you can 'see' what they are doing. Call this when they say 'look at my screen' or 'what am I doing'.".to_string(),
                parameters: None,
            },
            GeminiFunctionDeclaration {
                name: "search_clipboard".to_string(),
                description: "Searches the user's historical clipboard (copy history) for a keyword or recent items. Use this to find things they copied recently like links, snippets, or text.".to_string(),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The keyword to search for in clipboard history. Leave empty to get the most recent items."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of items to return (default 5)."
                        }
                    }
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
        "grep_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let pattern = args
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            match fs::read_to_string(path) {
                Ok(content) => {
                    let matches: Vec<serde_json::Value> = content
                        .lines()
                        .enumerate()
                        .filter(|(_, line)| line.to_lowercase().contains(&pattern))
                        .map(|(i, line)| json!({ "line": i + 1, "content": line }))
                        .collect();
                    json!({ "matches": matches })
                }
                Err(e) => json!({ "error": format!("Failed to read file for grep: {}", e) }),
            }
        }
        "edit_file_line" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args
                .get("line_number")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let new_content = args
                .get("new_content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if line_number == 0 {
                return json!({ "error": "Line number must be >= 1" });
            }

            match fs::read_to_string(path) {
                Ok(content) => {
                    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                    if line_number > lines.len() {
                        return json!({ "error": format!("File only has {} lines", lines.len()) });
                    }
                    lines[line_number - 1] = new_content.to_string();
                    match fs::write(path, lines.join("\n")) {
                        Ok(_) => {
                            json!({ "status": "success", "message": format!("Line {} updated", line_number) })
                        }
                        Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                    }
                }
                Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
            }
        }
        "insert_at_line" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args
                .get("line_number")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let content_to_insert = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

            if line_number == 0 {
                return json!({ "error": "Line number must be >= 1" });
            }

            match fs::read_to_string(path) {
                Ok(content) => {
                    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                    let idx = (line_number - 1).min(lines.len());
                    lines.insert(idx, content_to_insert.to_string());
                    match fs::write(path, lines.join("\n")) {
                        Ok(_) => {
                            json!({ "status": "success", "message": format!("Inserted at line {}", line_number) })
                        }
                        Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                    }
                }
                Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
            }
        }
        "delete_file_line" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args
                .get("line_number")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            if line_number == 0 {
                return json!({ "error": "Line number must be >= 1" });
            }

            match fs::read_to_string(path) {
                Ok(content) => {
                    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                    if line_number > lines.len() {
                        return json!({ "error": format!("File only has {} lines", lines.len()) });
                    }
                    lines.remove(line_number - 1);
                    match fs::write(path, lines.join("\n")) {
                        Ok(_) => {
                            json!({ "status": "success", "message": format!("Line {} deleted", line_number) })
                        }
                        Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                    }
                }
                Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
            }
        }
        "read_file_lines" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let start = args.get("start_line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let end = args.get("end_line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

            if start == 0 || end < start {
                return json!({ "error": "Invalid line range" });
            }

            match fs::read_to_string(path) {
                Ok(content) => {
                    let lines: Vec<String> = content
                        .lines()
                        .enumerate()
                        .filter(|(i, _)| i + 1 >= start && i + 1 <= end)
                        .map(|(_, s)| s.to_string())
                        .collect();
                    json!({ "lines": lines, "total_lines": content.lines().count() })
                }
                Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
            }
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
        "search_clipboard" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;

            match crate::database::queries::search_clipboard_history(db_connection, query, limit) {
                Ok(items) => json!({ "items": items }),
                Err(e) => json!({ "error": format!("Failed to search clipboard: {}", e) }),
            }
        }
        _ => json!({ "error": format!("Unknown synchronous tool: {}", name) }),
    }
}

//INFO: Execute an asynchronous tool call and return the result as JSON
pub async fn execute_tool_async(
    name: &str,
    args: &serde_json::Value,
    database: &crate::database::Database,
) -> serde_json::Value {
    match name {
        "get_weather" => {
            let location = args
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("Lagos");
            fetch_weather(location).await
        }
        "get_google_calendar_events" => {
            let time_min = args.get("time_min").and_then(|v| v.as_str()).unwrap_or("");
            let time_max = args.get("time_max").and_then(|v| v.as_str()).unwrap_or("");

            match crate::integrations::google_calendar::fetch_google_calendar_events(
                database, time_min, time_max,
            )
            .await
            {
                Ok(events) => json!({ "events": events }),
                Err(e) => json!({ "error": format!("Failed to fetch calendar: {}", e) }),
            }
        }
        "get_unread_emails" => {
            let max_results = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as u32;
            let query = args.get("query").and_then(|v| v.as_str());

            match crate::integrations::google_gmail::fetch_recent_emails_with_query(
                database,
                max_results,
                query,
            )
            .await
            {
                Ok(emails) => json!({ "emails": emails }),
                Err(e) => json!({ "error": format!("Failed to fetch emails: {}", e) }),
            }
        }
        "send_email" => {
            let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");

            match crate::integrations::google_gmail::send_email(database, to, subject, body).await {
                Ok(_) => json!({ "status": "success", "message": "Email sent." }),
                Err(e) => json!({ "error": format!("Failed up to send email: {}", e) }),
            }
        }
        "create_calendar_event" => {
            let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let description = args.get("description").and_then(|v| v.as_str());
            let start_time = args
                .get("start_time")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let end_time = args.get("end_time").and_then(|v| v.as_str()).unwrap_or("");
            let location = args.get("location").and_then(|v| v.as_str());

            match crate::integrations::google_calendar::create_calendar_event(
                database,
                summary,
                description,
                start_time,
                end_time,
                location,
            )
            .await
            {
                Ok(event) => json!({ "status": "success", "event": event }),
                Err(e) => json!({ "error": format!("Failed to create event: {}", e) }),
            }
        }
        "list_google_tasks" => {
            let max_results = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u32;
            match crate::integrations::google_tasks::list_tasks(database, max_results).await {
                Ok(tasks) => json!({ "tasks": tasks }),
                Err(e) => json!({ "error": format!("Failed to fetch tasks: {}", e) }),
            }
        }
        "create_google_task" => {
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let notes = args.get("notes").and_then(|v| v.as_str());
            let due = args.get("due").and_then(|v| v.as_str());

            match crate::integrations::google_tasks::create_task(database, title, notes, due).await
            {
                Ok(task) => json!({ "status": "success", "task": task }),
                Err(e) => json!({ "error": format!("Failed to create task: {}", e) }),
            }
        }
        "take_screenshot" => match crate::commands::vision::capture_primary_screen().await {
            Ok(b64) => {
                json!({ "status": "success", "image_data": b64, "message": "Screen captured. You can now see the image in the next turn." })
            }
            Err(e) => json!({ "error": format!("Failed to capture screen: {}", e) }),
        },
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
