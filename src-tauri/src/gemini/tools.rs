//INFO: Tool definitions and handlers for Gemini Function Calling
//NOTE: Single registry pattern — each tool's category and exec mode live next to its declaration

use crate::gemini::client::{GeminiFunctionDeclaration, GeminiTool};
use anyhow::Result;
use serde_json::json;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;
use walkdir::WalkDir;

//INFO: Which integration a tool belongs to. Core tools are always available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    Core,
    Google,
    Filesystem,
}

//INFO: Whether the dispatcher should route a tool to execute_tool_sync or execute_tool_async.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Sync,
    Async,
}

//INFO: One row in the tool registry. Single source of truth.
pub struct ToolDef {
    pub name: &'static str,
    pub category: ToolCategory,
    pub mode: ToolMode,
    pub build: fn() -> GeminiFunctionDeclaration,
}

//INFO: Result of a tool invocation. Attachment is used for binary payloads
//      (currently just screenshots) that should ride alongside the function
//      response as a separate inline_data part instead of being base64-stuffed
//      into the JSON response.
pub struct ToolResult {
    pub response: serde_json::Value,
    pub attachment: Option<ToolAttachment>,
}

pub struct ToolAttachment {
    pub mime_type: String,
    pub data: String,
}

impl ToolResult {
    pub fn ok(response: serde_json::Value) -> Self {
        Self { response, attachment: None }
    }
}

//INFO: The single source of truth for every tool Lumen exposes.
//      Adding a new tool means adding one entry here. The dispatcher
//      and the per-integration filters both read from this list.
fn registry() -> &'static [ToolDef] {
    &[
        // --- Core (always on) ---
        // NOTE: web search is no longer a function tool — it's handled by Gemini's
        // native `google_search` grounding, injected in chat.rs. The Tavily
        // `search_web` executor still exists (dormant) but is no longer advertised.
        ToolDef { name: "get_weather",           category: ToolCategory::Core, mode: ToolMode::Async, build: decl_get_weather },
        ToolDef { name: "take_screenshot",       category: ToolCategory::Core, mode: ToolMode::Async, build: decl_take_screenshot },
        ToolDef { name: "search_clipboard",      category: ToolCategory::Core, mode: ToolMode::Sync,  build: decl_search_clipboard },
        ToolDef { name: "set_reminder",          category: ToolCategory::Core, mode: ToolMode::Async, build: decl_set_reminder },
        ToolDef { name: "retrieve_past_memories",category: ToolCategory::Core, mode: ToolMode::Async, build: decl_retrieve_past_memories },
        ToolDef { name: "remember_this",         category: ToolCategory::Core, mode: ToolMode::Async, build: decl_remember_this },
        ToolDef { name: "search_memories",       category: ToolCategory::Core, mode: ToolMode::Async, build: decl_search_memories },
        ToolDef { name: "edit_memory",           category: ToolCategory::Core, mode: ToolMode::Async, build: decl_edit_memory },
        ToolDef { name: "forget_memory",         category: ToolCategory::Core, mode: ToolMode::Async, build: decl_forget_memory },
        ToolDef { name: "compress_memories",     category: ToolCategory::Core, mode: ToolMode::Async, build: decl_compress_memories },
        ToolDef { name: "view_runtime_logs",     category: ToolCategory::Core, mode: ToolMode::Sync,  build: decl_view_runtime_logs },

        // --- Google (gated by `google_enabled`) ---
        ToolDef { name: "get_google_calendar_events", category: ToolCategory::Google, mode: ToolMode::Async, build: decl_get_google_calendar_events },
        ToolDef { name: "get_unread_emails",          category: ToolCategory::Google, mode: ToolMode::Async, build: decl_get_unread_emails },
        ToolDef { name: "send_email",                 category: ToolCategory::Google, mode: ToolMode::Async, build: decl_send_email },
        ToolDef { name: "create_calendar_event",      category: ToolCategory::Google, mode: ToolMode::Async, build: decl_create_calendar_event },
        ToolDef { name: "delete_calendar_event",      category: ToolCategory::Google, mode: ToolMode::Async, build: decl_delete_calendar_event },

        // --- Filesystem / Obsidian (gated by `obsidian_enabled`) ---
        ToolDef { name: "read_file",              category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_read_file },
        ToolDef { name: "write_file",             category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_write_file },
        ToolDef { name: "list_files",             category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_list_files },
        ToolDef { name: "get_obsidian_vault_info",category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_get_obsidian_vault_info },
        ToolDef { name: "list_lumen_dir",         category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_list_lumen_dir },
        ToolDef { name: "read_lumen_file",        category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_read_lumen_file },
        ToolDef { name: "write_lumen_file",       category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_write_lumen_file },
        ToolDef { name: "append_lumen_file",      category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_append_lumen_file },
        ToolDef { name: "edit_lumen_file_line",   category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_edit_lumen_file_line },
        ToolDef { name: "create_lumen_dir",       category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_create_lumen_dir },
        ToolDef { name: "move_lumen_path",        category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_move_lumen_path },
        ToolDef { name: "delete_lumen_path",      category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_delete_lumen_path },
        ToolDef { name: "get_lumen_metadata",     category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_get_lumen_metadata },
        ToolDef { name: "search_lumen_dir",       category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_search_lumen_dir },
        ToolDef { name: "grep_file",              category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_grep_file },
        ToolDef { name: "edit_file_line",         category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_edit_file_line },
        ToolDef { name: "insert_at_line",         category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_insert_at_line },
        ToolDef { name: "delete_file_line",       category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_delete_file_line },
        ToolDef { name: "read_file_lines",        category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_read_file_lines },
        ToolDef { name: "get_file_metadata",      category: ToolCategory::Filesystem, mode: ToolMode::Sync,  build: decl_get_file_metadata },
        // Bulk traversal tools are async + spawn_blocking so they don't hold the chat DB lock.
        ToolDef { name: "search_notes",           category: ToolCategory::Filesystem, mode: ToolMode::Async, build: decl_search_notes },
        ToolDef { name: "search_filesystem",      category: ToolCategory::Filesystem, mode: ToolMode::Async, build: decl_search_filesystem },
        ToolDef { name: "search_vault",           category: ToolCategory::Filesystem, mode: ToolMode::Async, build: decl_search_vault },
        ToolDef { name: "list_recent_notes",      category: ToolCategory::Filesystem, mode: ToolMode::Async, build: decl_list_recent_notes },
        ToolDef { name: "search_by_tag",          category: ToolCategory::Filesystem, mode: ToolMode::Async, build: decl_search_by_tag },
    ]
}

//INFO: Returns true if the named tool should be dispatched via execute_tool_async.
//      Unknown tools default to Sync (they'll surface as "Unknown synchronous tool").
pub fn is_async(name: &str) -> bool {
    registry()
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.mode == ToolMode::Async)
        .unwrap_or(false)
}

//INFO: Builds the tool list advertised to Gemini for a given request, filtered
//      by which integrations the user has enabled. Core tools are always included.
pub fn build_tools_for_request(google_enabled: bool, obsidian_enabled: bool) -> Vec<GeminiTool> {
    let function_declarations: Vec<GeminiFunctionDeclaration> = registry()
        .iter()
        .filter(|t| match t.category {
            ToolCategory::Core => true,
            ToolCategory::Google => google_enabled,
            ToolCategory::Filesystem => obsidian_enabled,
        })
        .map(|t| (t.build)())
        .collect();

    vec![GeminiTool { function_declarations }]
}

// =============================================================================
// Declarations — each tool's JSON schema lives in its own tiny function so the
// registry stays scannable. Schemas use JSON Schema format hints where useful;
// stricter parse-validation happens in the executors.
// =============================================================================

fn decl_read_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "read_file".into(),
        description: "Reads the content of a local file (e.g., an Obsidian note or daily task list).".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The absolute path to the local file." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_write_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "write_file".into(),
        description: "Writes content to a local file. Use this for ticking tasks in daily notes OR updating vault content. Overwrites if it exists.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "The absolute path to the file." },
                "content": { "type": "string", "description": "The content to write to the local file." }
            },
            "required": ["path", "content"]
        })),
    }
}

fn decl_list_files() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "list_files".into(),
        description: "Lists files in a directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The absolute path to the directory." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_search_notes() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_notes".into(),
        description: "Searches for a keyword inside all markdown files in a directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":  { "type": "string",  "description": "The absolute path to the directory (usually the vault root)." },
                "query": { "type": "string",  "description": "The keyword to search for." },
                "limit": { "type": "integer", "description": "Max results to return (default 10)." },
                "modified_after": { "type": "string", "description": "Only search files modified after this ISO date." }
            },
            "required": ["path", "query"]
        })),
    }
}

fn decl_get_obsidian_vault_info() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_obsidian_vault_info".into(),
        description: "Gets information about the configured Obsidian vault, including its root path.".into(),
        parameters: None,
    }
}

fn decl_list_lumen_dir() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "list_lumen_dir".into(),
        description: "Lists files inside the configured Lumen directory in the user's Obsidian vault. Paths are relative to that Lumen directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Optional subdirectory relative to the configured Lumen directory.", "default": "" }
            }
        })),
    }
}

fn decl_read_lumen_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "read_lumen_file".into(),
        description: "Reads a text file from the configured Lumen directory. The path must be relative to that directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to the configured Lumen directory." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_write_lumen_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "write_lumen_file".into(),
        description: "Writes a text file inside the configured Lumen directory. Creates parent folders as needed. The path must be relative to that directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to the configured Lumen directory." },
                "content": { "type": "string", "description": "Text content to write." }
            },
            "required": ["path", "content"]
        })),
    }
}

fn decl_append_lumen_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "append_lumen_file".into(),
        description: "Appends text to a file inside the configured Lumen directory. Creates parent folders and the file if needed.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to the configured Lumen directory." },
                "content": { "type": "string", "description": "Text content to append." }
            },
            "required": ["path", "content"]
        })),
    }
}

fn decl_edit_lumen_file_line() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "edit_lumen_file_line".into(),
        description: "Replaces one line in a text file inside the configured Lumen directory. Read the file first when line numbers may be stale.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to the configured Lumen directory." },
                "line_number": { "type": "integer", "description": "1-based line number to replace." },
                "new_content": { "type": "string", "description": "Replacement text for that line." }
            },
            "required": ["path", "line_number", "new_content"]
        })),
    }
}

fn decl_create_lumen_dir() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "create_lumen_dir".into(),
        description: "Creates a folder inside the configured Lumen directory. The path must be relative to that directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path relative to the configured Lumen directory." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_move_lumen_path() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "move_lumen_path".into(),
        description: "Moves or renames a file/folder inside the configured Lumen directory. Both paths must be relative to that directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "from_path": { "type": "string", "description": "Existing relative file or folder path." },
                "to_path": { "type": "string", "description": "Destination relative file or folder path." }
            },
            "required": ["from_path", "to_path"]
        })),
    }
}

fn decl_delete_lumen_path() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "delete_lumen_path".into(),
        description: "Deletes a file or folder inside the configured Lumen directory. Recursive deletion is controlled by the recursive flag.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative file or folder path to delete." },
                "recursive": { "type": "boolean", "description": "Set true to delete a non-empty folder.", "default": false }
            },
            "required": ["path"]
        })),
    }
}

fn decl_get_lumen_metadata() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_lumen_metadata".into(),
        description: "Gets metadata for a file or folder inside the configured Lumen directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative file or folder path." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_search_lumen_dir() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_lumen_dir".into(),
        description: "Searches text files inside the configured Lumen directory and returns matching file paths, line numbers, and snippets.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Text to search for." },
                "path": { "type": "string", "description": "Optional relative subdirectory to search.", "default": "" },
                "limit": { "type": "integer", "description": "Maximum matches to return.", "default": 25 },
                "file_extension": { "type": "string", "description": "Filter by file extension (e.g. 'md', 'txt')." },
                "modified_after": { "type": "string", "description": "Only search files modified after this ISO date." }
            },
            "required": ["query"]
        })),
    }
}

fn decl_get_weather() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_weather".into(),
        description: "Gets the current weather for a location.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "location": { "type": "string", "description": "The city or location." }
            },
            "required": ["location"]
        })),
    }
}

fn decl_get_google_calendar_events() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_google_calendar_events".into(),
        description: "Lists Google Calendar events for a specific time range.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "time_min": { "type": "string", "format": "date-time", "description": "Start time in RFC3339 format (e.g. '2026-01-20T00:00:00Z')." },
                "time_max": { "type": "string", "format": "date-time", "description": "End time in RFC3339 format." }
            },
            "required": ["time_min", "time_max"]
        })),
    }
}

fn decl_get_unread_emails() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_unread_emails".into(),
        description: "Lists recent emails from Gmail. Can filter by query (e.g. 'newer_than:1d', 'after:2026/01/20', 'from:person@example.com').".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Maximum number of emails to fetch (default 5)." },
                "query":       { "type": "string", "description": "Gmail search query. For today's emails use 'newer_than:1d'. Default is 'is:unread inbox'." }
            }
        })),
    }
}

fn decl_send_email() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "send_email".into(),
        description: "Sends an email using Gmail.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "to":      { "type": "string", "description": "Recipient email address." },
                "subject": { "type": "string", "description": "Email subject." },
                "body":    { "type": "string", "description": "Email body content." }
            },
            "required": ["to", "subject", "body"]
        })),
    }
}

fn decl_create_calendar_event() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "create_calendar_event".into(),
        description: "Creates a new event in the user's primary Google Calendar. IMPORTANT: Use the current year and the user's timezone offset from the 'ISO' time provided in CONTEXT (e.g. '2026-01-20T14:00:00+01:00').".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "summary":     { "type": "string", "description": "Event title." },
                "description": { "type": "string", "description": "Event description." },
                "start_time":  { "type": "string", "format": "date-time", "description": "Start time in RFC3339 format with offset (e.g. '2026-01-20T14:00:00+01:00')." },
                "end_time":    { "type": "string", "format": "date-time", "description": "End time in RFC3339 format with offset." },
                "location":    { "type": "string", "description": "Physical or virtual location." }
            },
            "required": ["summary", "start_time", "end_time"]
        })),
    }
}

fn decl_delete_calendar_event() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "delete_calendar_event".into(),
        description: "Deletes an event from the user's primary Google Calendar using its unique event ID. IMPORTANT: You must first use 'get_google_calendar_events' to find the 'id' of the event you want to delete.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "event_id": { "type": "string", "description": "The unique ID of the event to delete." }
            },
            "required": ["event_id"]
        })),
    }
}

fn decl_grep_file() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "grep_file".into(),
        description: "Searches for a pattern in a file and returns matching lines with line numbers.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Absolute path to the file." },
                "pattern": { "type": "string", "description": "The string to search for (case-insensitive)." }
            },
            "required": ["path", "pattern"]
        })),
    }
}

fn decl_edit_file_line() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "edit_file_line".into(),
        description: "Replaces a specific line in a file by line number (1-indexed).".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":        { "type": "string",  "description": "Absolute path to the file." },
                "line_number": { "type": "integer", "minimum": 1, "description": "The 1-based line number to replace." },
                "new_content": { "type": "string",  "description": "The new content for that line." }
            },
            "required": ["path", "line_number", "new_content"]
        })),
    }
}

fn decl_insert_at_line() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "insert_at_line".into(),
        description: "Inserts a new line at a specific line number (1-indexed). Everything else shifts down.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":        { "type": "string",  "description": "Absolute path to the file." },
                "line_number": { "type": "integer", "minimum": 1, "description": "The 1-based line number to insert at." },
                "content":     { "type": "string",  "description": "The content to insert." }
            },
            "required": ["path", "line_number", "content"]
        })),
    }
}

fn decl_delete_file_line() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "delete_file_line".into(),
        description: "Deletes a specific line from a file by line number (1-indexed).".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":        { "type": "string",  "description": "Absolute path to the file." },
                "line_number": { "type": "integer", "minimum": 1, "description": "The 1-based line number to delete." }
            },
            "required": ["path", "line_number"]
        })),
    }
}

fn decl_read_file_lines() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "read_file_lines".into(),
        description: "Reads a specific range of lines from a file (1-indexed). Use this to verify context before editing.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":       { "type": "string",  "description": "Absolute path to the file." },
                "start_line": { "type": "integer", "minimum": 1, "description": "The first line to read." },
                "end_line":   { "type": "integer", "minimum": 1, "description": "The last line to read." }
            },
            "required": ["path", "start_line", "end_line"]
        })),
    }
}

fn decl_take_screenshot() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "take_screenshot".into(),
        description: "Captures a screenshot of the user's primary screen so you can 'see' what they are doing. Call this when they say 'look at my screen' or 'what am I doing'.".into(),
        parameters: None,
    }
}

fn decl_search_clipboard() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_clipboard".into(),
        description: "Searches the user's historical clipboard (copy history) for a keyword or recent items. Use this to find things they copied recently like links, snippets, or text.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query":        { "type": "string",  "description": "The keyword to search for in clipboard history." },
                "after":        { "type": "string",  "description": "Only items copied after this ISO date." },
                "before":       { "type": "string",  "description": "Only items copied before this ISO date." },
                "content_type": { "type": "string",  "enum": ["text", "image"], "description": "Filter by content type." },
                "limit":        { "type": "integer", "minimum": 1, "maximum": 50, "description": "Maximum number of items to return (default 5)." }
            }
        })),
    }
}

fn decl_get_file_metadata() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "get_file_metadata".into(),
        description: "Gets metadata (size, last modified, creation time) for a local file.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file." }
            },
            "required": ["path"]
        })),
    }
}

fn decl_search_filesystem() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_filesystem".into(),
        description: "Recursively searches for files matching a filename or extension in a directory.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":  { "type": "string",  "description": "Directory to search in." },
                "query": { "type": "string",  "description": "The filename or extension to search for (e.g. 'resume.pdf' or '.js')." },
                "limit": { "type": "integer", "description": "Max results to return (default 20)." },
                "max_depth": { "type": "integer", "description": "Recursion depth (default 5)." },
                "file_extension": { "type": "string", "description": "Filter by extension." }
            },
            "required": ["path", "query"]
        })),
    }
}

fn decl_search_vault() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_vault".into(),
        description: "PRIMARY vault search. Searches the whole Obsidian vault for a query and returns RANKED matches — each with the file path, line number, and a snippet of the matching line — so you can see exactly where things are. Multi-word queries match notes/lines mentioning those words (order-independent). Use this for 'what did I write about X', 'find my notes on Y'. Then call read_file_lines on a hit to read around it. The vault root is auto-detected; only pass `path` to scope to a subfolder.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keywords or phrase to search for. Multiple words are matched independently and ranked by how many hit." },
                "path":  { "type": "string", "description": "Optional. A subfolder (absolute path) to scope the search to. Omit to search the entire vault." },
                "max_results": { "type": "integer", "description": "Optional. Max number of ranked matches to return (default 12)." },
                "file_extension": { "type": "string", "description": "Filter by file extension (e.g. 'md', 'txt')." },
                "modified_after": { "type": "string", "description": "Only search files modified after this ISO date." },
                "modified_before": { "type": "string", "description": "Only search files modified before this ISO date." }
            },
            "required": ["query"]
        })),
    }
}

fn decl_list_recent_notes() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "list_recent_notes".into(),
        description: "Lists the most recently modified notes in the vault (newest first), each with its path, a title, and when it was last edited. Use for 'what was I working on', 'my latest notes', 'recent journal entries'. The vault root is auto-detected; pass `path` only to scope to a subfolder.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "path":  { "type": "string", "description": "Optional. A subfolder (absolute path) to scope to. Omit for the whole vault." },
                "limit": { "type": "integer", "description": "Optional. How many recent notes to return (default 10)." }
            },
            "required": []
        })),
    }
}

fn decl_search_by_tag() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_by_tag".into(),
        description: "Finds notes carrying a given Obsidian tag — either an inline #tag or a frontmatter `tags:` entry. Use when the user references a tag or category like 'my #recipe notes' or 'everything tagged project'. Returns the matching file paths. The vault root is auto-detected.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "tag":  { "type": "string", "description": "The tag to search for, with or without the leading '#' (e.g. 'recipe' or '#recipe')." },
                "path": { "type": "string", "description": "Optional. A subfolder (absolute path) to scope to. Omit for the whole vault." }
            },
            "required": ["tag"]
        })),
    }
}

fn decl_set_reminder() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "set_reminder".into(),
        description: "Sets a reminder that fires a system notification at a specific time. Use this when the user says things like 'remind me', 'don't let me forget', or 'alert me at'. Always resolve relative times (e.g. 'in 2 hours', 'at 3pm') to an absolute RFC3339 timestamp using the current time from CONTEXT.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "The reminder message to show the user when it fires." },
                "due_at":  { "type": "string", "format": "date-time", "description": "When to trigger the reminder, as an RFC3339 timestamp with timezone offset (e.g. '2026-04-01T15:00:00+01:00')." }
            },
            "required": ["content", "due_at"]
        })),
    }
}

fn decl_retrieve_past_memories() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "retrieve_past_memories".into(),
        description: "Searches Lumen's long-term memory of past conversations and learned facts about the user. Call this when the user references something from the past ('remember when…', 'what did I tell you about…', 'who is X?', 'what do I like…') OR when answering a question would clearly benefit from knowing the user's preferences, history, or relationships. Do NOT call for trivial chit-chat.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Semantic search query (embeds and does vector KNN). Optional if keyword is provided." },
                "keyword": { "type": "string", "description": "Exact text search in memory content (case-insensitive). Faster than query—no embedding needed." },
                "type": { "type": "string", "enum": ["observation", "reflection", "entity", "preference", "daily_summary"], "description": "Filter to a specific memory type." },
                "after": { "type": "string", "description": "Only memories created after this ISO date (e.g. '2026-06-01')." },
                "before": { "type": "string", "description": "Only memories created before this ISO date." },
                "min_importance": { "type": "number", "description": "Minimum importance score (1-10)." },
                "limit": { "type": "integer", "description": "Max results to return (default 15)." }
            }
        })),
    }
}

fn decl_remember_this() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "remember_this".into(),
        description: "Explicitly save a durable fact about the user to long-term memory. Call this when the user asks you to remember something ('remember that…', 'don't forget…', 'keep in mind…') or volunteers a stable personal fact worth recalling later (a preference, an important date, a relationship, a goal). Do NOT use it for fleeting, in-the-moment details. Write the fact as a clear standalone sentence.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "The fact to remember, as a standalone sentence (e.g. 'Sijibomi is allergic to peanuts')." },
                "type": { "type": "string", "enum": ["preference", "entity", "observation"], "description": "preference = a like/dislike/goal; entity = a named person/project/thing; observation = a general fact. Default observation." },
                "importance": { "type": "integer", "description": "1-10, how important this is to remember. Default 8 for an explicit user request." }
            },
            "required": ["content"]
        })),
    }
}

fn decl_search_memories() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "search_memories".into(),
        description: "Search your own long-term memory and get back the matching entries WITH their ids, so you can curate them (edit_memory / forget_memory). Use this when the user asks you to review, correct, update, or delete something you remember ('what do you remember about my job?', 'that's wrong, fix it', 'forget that'). Distinct from retrieve_past_memories, which is for silently recalling context to answer a question — use search_memories when memory itself is the subject and you may change it.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Semantic search query (embeds and does vector KNN). Optional if keyword is provided." },
                "keyword": { "type": "string", "description": "Exact text search in memory content (case-insensitive). Faster than query—no embedding needed." },
                "type": { "type": "string", "enum": ["observation", "reflection", "entity", "preference", "daily_summary"], "description": "Filter to a specific memory type." },
                "after": { "type": "string", "description": "Only memories created after this ISO date (e.g. '2026-06-01')." },
                "before": { "type": "string", "description": "Only memories created before this ISO date." },
                "min_importance": { "type": "number", "description": "Minimum importance score (1-10)." },
                "limit": { "type": "integer", "description": "Max results to return (default 15)." }
            }
        })),
    }
}

fn decl_edit_memory() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "edit_memory".into(),
        description: "Correct or update the text of an existing memory. Get the memory's id from search_memories FIRST. Use when a remembered fact is wrong or out of date ('actually I moved to Berlin', 'update that — I switched jobs'). This rewrites the memory and re-indexes it so future recall is accurate.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "id":      { "type": "string", "description": "The id of the memory to edit (from search_memories)." },
                "content": { "type": "string", "description": "The new, corrected text as a standalone sentence." }
            },
            "required": ["id", "content"]
        })),
    }
}

fn decl_forget_memory() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "forget_memory".into(),
        description: "Permanently delete a single memory. Get its id from search_memories FIRST. Use when the user asks you to forget something specific ('forget that I said that', 'delete what you know about X'). Irreversible — only delete the memory the user actually meant.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The id of the memory to delete (from search_memories)." }
            },
            "required": ["id"]
        })),
    }
}

fn decl_compress_memories() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "compress_memories".into(),
        description: "Compresses multiple fragmented memories into a single dense memory. Use this to clean up your database when you have many overlapping or outdated facts. Pass the list of old memory IDs to delete, and the new compressed summary fact.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "ids_to_delete": { 
                    "type": "array", 
                    "items": { "type": "string" },
                    "description": "The IDs of the scattered memories getting merged (get these from search_memories first)."
                },
                "new_content": { "type": "string", "description": "The single compressed summary containing the facts from those old memories." }
            },
            "required": ["ids_to_delete", "new_content"]
        })),
    }
}

fn decl_view_runtime_logs() -> GeminiFunctionDeclaration {
    GeminiFunctionDeclaration {
        name: "view_runtime_logs".into(),
        description: "Read your OWN recent runtime logs (tool calls, results, errors, timings, API activity) to debug yourself. Call this when the user reports a bug or asks 'why did that fail / what just went wrong', or when you need to inspect what happened internally on recent turns. Returns the most recent log lines, oldest first.".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional case-insensitive substring to narrow the logs (e.g. 'error', 'Tool', a tool name, 'memory'). Omit for everything recent." },
                "limit":  { "type": "integer", "minimum": 1, "maximum": 500, "description": "Max number of recent lines to return (default 100)." }
            }
        })),
    }
}

// =============================================================================
// Validation helpers — surface clear errors back to the model so it can correct
// =============================================================================

//INFO: Validate an RFC3339 timestamp (Google APIs' required format). Returns a
//      tool-error JSON if invalid, so the LLM gets actionable feedback.
fn validate_rfc3339(value: &str, field: &str) -> Result<(), serde_json::Value> {
    if value.is_empty() {
        return Err(json!({
            "error": format!("Missing required field '{}'.", field)
        }));
    }
    match chrono::DateTime::parse_from_rfc3339(value) {
        Ok(_) => Ok(()),
        Err(e) => Err(json!({
            "error": format!(
                "Invalid timestamp '{}' for field '{}': {}. Use RFC3339 with offset, e.g. '2026-04-01T15:00:00+01:00' or '2026-04-01T15:00:00Z'.",
                value, field, e
            )
        })),
    }
}

//INFO: Validate that an email looks vaguely sane before handing to Gmail.
fn validate_email(value: &str, field: &str) -> Result<(), serde_json::Value> {
    if value.is_empty() {
        return Err(json!({ "error": format!("Missing required field '{}'.", field) }));
    }
    // Tiny check — Gmail will do real validation server-side.
    if !value.contains('@') || !value.contains('.') {
        return Err(json!({
            "error": format!("Invalid email '{}' for field '{}'. Must look like 'name@example.com'.", value, field)
        }));
    }
    Ok(())
}

fn configured_lumen_root(obsidian_config: Option<&serde_json::Value>) -> Result<PathBuf, String> {
    let config = obsidian_config.ok_or_else(|| "Obsidian vault not configured.".to_string())?;
    let vault_path = config
        .get("vault_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Obsidian vault path is not configured.".to_string())?;
    let lumen_dir = config
        .get("lumen_dir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Lumen directory is not configured in the Obsidian integration.".to_string())?;

    Ok(Path::new(vault_path).join(lumen_dir))
}

fn resolve_lumen_path(obsidian_config: Option<&serde_json::Value>, relative_path: &str) -> Result<PathBuf, String> {
    let root = configured_lumen_root(obsidian_config)?;
    let relative = Path::new(relative_path);

    if relative.is_absolute() {
        return Err("Path must be relative to the configured Lumen directory.".to_string());
    }

    if relative
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err("Path cannot escape the configured Lumen directory.".to_string());
    }

    Ok(root.join(relative))
}

// =============================================================================
// Sync executor
// =============================================================================

//INFO: Execute a synchronous tool call and return the result.
pub fn execute_tool_sync(
    name: &str,
    args: &serde_json::Value,
    obsidian_config: Option<&serde_json::Value>,
    db_connection: &rusqlite::Connection,
    _app_handle: &tauri::AppHandle,
) -> ToolResult {
    let value = match name {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            // SAFETY: Don't read files larger than 500KB to avoid API context explosion or binary bloat
            if let Ok(meta) = fs::metadata(path) {
                if meta.len() > 500_000 {
                    json!({ "error": "File is too large to read (limit: 500KB). Use get_file_metadata first or read selective lines." })
                } else {
                    match fs::read_to_string(path) {
                        Ok(content) => json!({ "content": content }),
                        Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                    }
                }
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => json!({ "content": content }),
                    Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                }
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
                            if e.path().is_dir() { format!("{}/", name) } else { name }
                        })
                        .collect();
                    json!({ "entries": files, "current_path": path })
                }
                Err(e) => json!({ "error": format!("Failed to list directory: {}", e) }),
            }
        }
        "get_obsidian_vault_info" => {
            if let Some(config) = obsidian_config {
                json!({
                    "vault_path": config.get("vault_path"),
                    "daily_notes_folder": config.get("daily_notes_path").and_then(|v| v.as_str()).unwrap_or(""),
                    "daily_notes_format": config.get("daily_notes_format").and_then(|v| v.as_str()).unwrap_or("YYYY-MM-DD"),
                    "lumen_dir": config.get("lumen_dir").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "configured"
                })
            } else {
                json!({ "error": "Obsidian vault not configured in settings." })
            }
        }
        "list_lumen_dir" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => match fs::read_dir(&path) {
                    Ok(entries) => {
                        let files: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().into_owned();
                                if e.path().is_dir() { format!("{}/", name) } else { name }
                            })
                            .collect();
                        json!({
                            "entries": files,
                            "relative_path": relative_path,
                            "absolute_path": path.to_string_lossy()
                        })
                    }
                    Err(e) => json!({ "error": format!("Failed to list Lumen directory: {}", e) }),
                },
                Err(e) => json!({ "error": e }),
            }
        }
        "read_lumen_file" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => {
                    if let Ok(meta) = fs::metadata(&path) {
                        if meta.len() > 500_000 {
                            json!({ "error": "File is too large to read (limit: 500KB)." })
                        } else {
                            match fs::read_to_string(&path) {
                                Ok(content) => json!({ "content": content, "path": relative_path }),
                                Err(e) => json!({ "error": format!("Failed to read Lumen file: {}", e) }),
                            }
                        }
                    } else {
                        match fs::read_to_string(&path) {
                            Ok(content) => json!({ "content": content, "path": relative_path }),
                            Err(e) => json!({ "error": format!("Failed to read Lumen file: {}", e) }),
                        }
                    }
                }
                Err(e) => json!({ "error": e }),
            }
        }
        "write_lumen_file" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            return ToolResult::ok(json!({ "error": format!("Failed to create parent directory: {}", e) }));
                        }
                    }
                    match fs::write(&path, content) {
                        Ok(_) => json!({
                            "status": "success",
                            "path": relative_path,
                            "absolute_path": path.to_string_lossy()
                        }),
                        Err(e) => json!({ "error": format!("Failed to write Lumen file: {}", e) }),
                    }
                }
                Err(e) => json!({ "error": e }),
            }
        }
        "append_lumen_file" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            return ToolResult::ok(json!({ "error": format!("Failed to create parent directory: {}", e) }));
                        }
                    }
                    match fs::OpenOptions::new().create(true).append(true).open(&path) {
                        Ok(mut file) => {
                            use std::io::Write;
                            match file.write_all(content.as_bytes()) {
                                Ok(_) => json!({ "status": "success", "path": relative_path }),
                                Err(e) => json!({ "error": format!("Failed to append Lumen file: {}", e) }),
                            }
                        }
                        Err(e) => json!({ "error": format!("Failed to open Lumen file for append: {}", e) }),
                    }
                }
                Err(e) => json!({ "error": e }),
            }
        }
        "edit_lumen_file_line" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args.get("line_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let new_content = args.get("new_content").and_then(|v| v.as_str()).unwrap_or("");
            if line_number == 0 {
                json!({ "error": "Line number must be >= 1" })
            } else {
                match resolve_lumen_path(obsidian_config, relative_path) {
                    Ok(path) => match fs::read_to_string(&path) {
                        Ok(content) => {
                            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                            if line_number > lines.len() {
                                json!({ "error": format!("File only has {} lines", lines.len()) })
                            } else {
                                lines[line_number - 1] = new_content.to_string();
                                match fs::write(&path, lines.join("\n")) {
                                    Ok(_) => json!({ "status": "success", "path": relative_path, "message": format!("Line {} updated", line_number) }),
                                    Err(e) => json!({ "error": format!("Failed to write Lumen file: {}", e) }),
                                }
                            }
                        }
                        Err(e) => json!({ "error": format!("Failed to read Lumen file: {}", e) }),
                    },
                    Err(e) => json!({ "error": e }),
                }
            }
        }
        "create_lumen_dir" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => match fs::create_dir_all(&path) {
                    Ok(_) => json!({ "status": "success", "path": relative_path, "absolute_path": path.to_string_lossy() }),
                    Err(e) => json!({ "error": format!("Failed to create Lumen directory: {}", e) }),
                },
                Err(e) => json!({ "error": e }),
            }
        }
        "move_lumen_path" => {
            let from_path = args.get("from_path").and_then(|v| v.as_str()).unwrap_or("");
            let to_path = args.get("to_path").and_then(|v| v.as_str()).unwrap_or("");
            match (
                resolve_lumen_path(obsidian_config, from_path),
                resolve_lumen_path(obsidian_config, to_path),
            ) {
                (Ok(from), Ok(to)) => {
                    if let Some(parent) = to.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            return ToolResult::ok(json!({ "error": format!("Failed to create destination parent directory: {}", e) }));
                        }
                    }
                    match fs::rename(&from, &to) {
                        Ok(_) => json!({ "status": "success", "from_path": from_path, "to_path": to_path }),
                        Err(e) => json!({ "error": format!("Failed to move Lumen path: {}", e) }),
                    }
                }
                (Err(e), _) | (_, Err(e)) => json!({ "error": e }),
            }
        }
        "delete_lumen_path" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => match fs::metadata(&path) {
                    Ok(meta) if meta.is_dir() && recursive => match fs::remove_dir_all(&path) {
                        Ok(_) => json!({ "status": "success", "path": relative_path }),
                        Err(e) => json!({ "error": format!("Failed to delete Lumen directory: {}", e) }),
                    },
                    Ok(meta) if meta.is_dir() => match fs::remove_dir(&path) {
                        Ok(_) => json!({ "status": "success", "path": relative_path }),
                        Err(e) => json!({ "error": format!("Failed to delete Lumen directory. Use recursive=true for non-empty folders: {}", e) }),
                    },
                    Ok(_) => match fs::remove_file(&path) {
                        Ok(_) => json!({ "status": "success", "path": relative_path }),
                        Err(e) => json!({ "error": format!("Failed to delete Lumen file: {}", e) }),
                    },
                    Err(e) => json!({ "error": format!("Failed to inspect Lumen path: {}", e) }),
                },
                Err(e) => json!({ "error": e }),
            }
        }
        "get_lumen_metadata" => {
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match resolve_lumen_path(obsidian_config, relative_path) {
                Ok(path) => match fs::metadata(&path) {
                    Ok(meta) => {
                        use std::time::SystemTime;
                        let format_time = |t: std::io::Result<SystemTime>| {
                            t.ok()
                                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                        };
                        json!({
                            "path": relative_path,
                            "absolute_path": path.to_string_lossy(),
                            "size_bytes": meta.len(),
                            "is_dir": meta.is_dir(),
                            "modified": format_time(meta.modified()),
                            "created": format_time(meta.created()),
                        })
                    }
                    Err(e) => json!({ "error": format!("Failed to get Lumen metadata: {}", e) }),
                },
                Err(e) => json!({ "error": e }),
            }
        }
        "search_lumen_dir" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let relative_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(25).clamp(1, 100) as usize;
            let extension = args.get("file_extension").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
            let modified_after = args.get("modified_after").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.timestamp());

            if query.is_empty() {
                json!({ "error": "Field 'query' is required." })
            } else {
                match resolve_lumen_path(obsidian_config, relative_path) {
                    Ok(root) => {
                        let base = configured_lumen_root(obsidian_config).ok();
                        let mut matches = Vec::new();
                        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                            if matches.len() >= limit {
                                break;
                            }
                            if !entry.file_type().is_file() || is_hidden_path(entry.path()) {
                                continue;
                            }
                            if let Some(ref ext) = extension {
                                if entry.path().extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some(ext.clone()) {
                                    continue;
                                }
                            }
                            if let Some(min_time) = modified_after {
                                let mtime = fs::metadata(entry.path()).ok()
                                    .and_then(|m| m.modified().ok())
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                if (mtime as i64) < min_time {
                                    continue;
                                }
                            }

                            let Ok(content) = fs::read_to_string(entry.path()) else {
                                continue;
                            };
                            for (line_index, line) in content.lines().enumerate() {
                                if line.to_lowercase().contains(&query) {
                                    let path = base
                                        .as_ref()
                                        .and_then(|b| entry.path().strip_prefix(b).ok())
                                        .unwrap_or(entry.path())
                                        .to_string_lossy()
                                        .to_string();
                                    matches.push(json!({
                                        "path": path,
                                        "line": line_index + 1,
                                        "snippet": line.trim()
                                    }));
                                    if matches.len() >= limit {
                                        break;
                                    }
                                }
                            }
                        }
                        json!({ "matches": matches, "query": query })
                    }
                    Err(e) => json!({ "error": e }),
                }
            }
        }
        "grep_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
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
            let line_number = args.get("line_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let new_content = args.get("new_content").and_then(|v| v.as_str()).unwrap_or("");
            if line_number == 0 {
                json!({ "error": "Line number must be >= 1" })
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => {
                        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        if line_number > lines.len() {
                            json!({ "error": format!("File only has {} lines", lines.len()) })
                        } else {
                            lines[line_number - 1] = new_content.to_string();
                            match fs::write(path, lines.join("\n")) {
                                Ok(_) => json!({ "status": "success", "message": format!("Line {} updated", line_number) }),
                                Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                            }
                        }
                    }
                    Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                }
            }
        }
        "insert_at_line" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args.get("line_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let content_to_insert = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if line_number == 0 {
                json!({ "error": "Line number must be >= 1" })
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => {
                        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        let idx = (line_number - 1).min(lines.len());
                        lines.insert(idx, content_to_insert.to_string());
                        match fs::write(path, lines.join("\n")) {
                            Ok(_) => json!({ "status": "success", "message": format!("Inserted at line {}", line_number) }),
                            Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                        }
                    }
                    Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                }
            }
        }
        "delete_file_line" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line_number = args.get("line_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if line_number == 0 {
                json!({ "error": "Line number must be >= 1" })
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => {
                        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        if line_number > lines.len() {
                            json!({ "error": format!("File only has {} lines", lines.len()) })
                        } else {
                            lines.remove(line_number - 1);
                            match fs::write(path, lines.join("\n")) {
                                Ok(_) => json!({ "status": "success", "message": format!("Line {} deleted", line_number) }),
                                Err(e) => json!({ "error": format!("Failed to write file: {}", e) }),
                            }
                        }
                    }
                    Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                }
            }
        }
        "read_file_lines" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let start = args.get("start_line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let end = args.get("end_line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            if start == 0 || end < start {
                json!({ "error": "Invalid line range" })
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => {
                        let lines: Vec<String> = content
                            .lines()
                            .enumerate()
                            .filter(|(i, _)| i + 1 >= start && *i < end)
                            .map(|(_, s)| s.to_string())
                            .collect();
                        json!({ "lines": lines, "total_lines": content.lines().count() })
                    }
                    Err(e) => json!({ "error": format!("Failed to read file: {}", e) }),
                }
            }
        }
        "search_clipboard" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
            let after = args.get("after").and_then(|v| v.as_str());
            let before = args.get("before").and_then(|v| v.as_str());
            let context_type = args.get("content_type").and_then(|v| v.as_str());

            match crate::database::queries::search_clipboard_history_filtered(db_connection, query, after, before, context_type, limit) {
                Ok(items) => json!({ "items": items }),
                Err(e) => json!({ "error": format!("Failed to search clipboard: {}", e) }),
            }
        }
        "get_file_metadata" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match fs::metadata(path) {
                Ok(meta) => {
                    use std::time::SystemTime;
                    let format_time = |t: Result<SystemTime, _>| {
                        t.ok()
                            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                    };
                    json!({
                        "size_bytes": meta.len(),
                        "is_dir": meta.is_dir(),
                        "modified": format_time(meta.modified()),
                        "created": format_time(meta.created()),
                    })
                }
                Err(e) => json!({ "error": format!("Failed to get metadata: {}", e) }),
            }
        }
        "view_runtime_logs" => {
            let filter = args
                .get("filter")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(100)
                .clamp(1, 500) as usize;
            let lines = crate::logbuf::tail(limit, filter);
            if lines.is_empty() {
                json!({ "message": "No matching log lines in the buffer yet." })
            } else {
                json!({ "count": lines.len(), "log_lines": lines })
            }
        }
        _ => json!({ "error": format!("Unknown synchronous tool: {}", name) }),
    };

    ToolResult::ok(value)
}

// =============================================================================
// Async executor
// =============================================================================

//INFO: Execute an asynchronous tool call and return the result.
//      No approval gate — destructive actions are confirmed conversationally
//      by Lumen (see the CONFIRMATION RULE in the system prompt). When the
//      model finally invokes the tool, the user has already said yes.
//INFO: Resolves the Obsidian vault root from the stored integration config so the
//      vault-search tools can default to "the whole vault" without the model
//      having to know (or guess) the absolute path. Locks the DB connection
//      briefly — safe to call from async tool executors (they don't hold it).
fn vault_root_from_db(database: &crate::database::Database) -> Option<String> {
    let conn = database.connection.lock();
    let integration = crate::database::queries::get_integration(&conn, "obsidian")
        .ok()
        .flatten()?;
    let config = integration.config?;
    let json: serde_json::Value = serde_json::from_str(&config).ok()?;
    json.get("vault_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

//INFO: Should this path be skipped during vault traversal? (dotfiles, .obsidian,
//      .trash, .git — anything starting with a dot in any component.)
fn is_hidden_path(path: &std::path::Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_str().map(|s| s.starts_with('.')).unwrap_or(false))
}

//INFO: Helper to build a MemorySearchFilter from JSON tool arguments
fn filter_from_args(args: &serde_json::Value) -> crate::memory::core::MemorySearchFilter {
    use crate::memory::core::{MemorySearchFilter, MemoryType};
    MemorySearchFilter {
        memory_type: args.get("type").and_then(|v| v.as_str()).and_then(MemoryType::from_str),
        keyword: args.get("keyword").and_then(|v| v.as_str()).map(|s| s.to_string()),
        after: args.get("after").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc)),
        before: args.get("before").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc)),
        min_importance: args.get("min_importance").and_then(|v| v.as_f64()),
        limit: args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize),
    }
}

pub async fn execute_tool_async(
    name: &str,
    args: &serde_json::Value,
    database: &crate::database::Database,
    app_handle: &tauri::AppHandle,
) -> ToolResult {
    match name {
        "get_weather" => {
            let location = args.get("location").and_then(|v| v.as_str()).unwrap_or("Lagos");
            ToolResult::ok(fetch_weather(location).await)
        }
        "get_google_calendar_events" => {
            let time_min = args.get("time_min").and_then(|v| v.as_str()).unwrap_or("");
            let time_max = args.get("time_max").and_then(|v| v.as_str()).unwrap_or("");
            if let Err(e) = validate_rfc3339(time_min, "time_min") { return ToolResult::ok(e); }
            if let Err(e) = validate_rfc3339(time_max, "time_max") { return ToolResult::ok(e); }

            let value = match crate::integrations::google_calendar::fetch_google_calendar_events(
                database, time_min, time_max,
            ).await {
                Ok(events) => json!({ "events": events }),
                Err(e) => json!({ "error": format!("Failed to fetch calendar: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "get_unread_emails" => {
            let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
            let query = args.get("query").and_then(|v| v.as_str());

            let value = match crate::integrations::google_gmail::fetch_recent_emails_with_query(
                database, max_results, query,
            ).await {
                Ok(emails) => json!({ "emails": emails }),
                Err(e) => json!({ "error": format!("Failed to fetch emails: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "send_email" => {
            let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
            if let Err(e) = validate_email(to, "to") { return ToolResult::ok(e); }

            let value = match crate::integrations::google_gmail::send_email(database, to, subject, body).await {
                Ok(_) => json!({ "status": "success", "message": "Email sent." }),
                Err(e) => json!({ "error": format!("Failed to send email: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "create_calendar_event" => {
            let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let description = args.get("description").and_then(|v| v.as_str());
            let start_time = args.get("start_time").and_then(|v| v.as_str()).unwrap_or("");
            let end_time = args.get("end_time").and_then(|v| v.as_str()).unwrap_or("");
            let location = args.get("location").and_then(|v| v.as_str());
            if let Err(e) = validate_rfc3339(start_time, "start_time") { return ToolResult::ok(e); }
            if let Err(e) = validate_rfc3339(end_time, "end_time") { return ToolResult::ok(e); }

            let value = match crate::integrations::google_calendar::create_calendar_event(
                database, summary, description, start_time, end_time, location,
            ).await {
                Ok(event) => json!({ "status": "success", "event": event }),
                Err(e) => json!({ "error": format!("Failed to create event: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "delete_calendar_event" => {
            let event_id = args.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
            let value = match crate::integrations::google_calendar::delete_calendar_event(database, event_id).await {
                Ok(_) => json!({ "status": "success", "message": "Event deleted successfully." }),
                Err(e) => json!({ "error": format!("Failed to delete event: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "take_screenshot" => {
            match crate::commands::vision::capture_primary_screen().await {
                Ok(b64) => ToolResult {
                    // image_data is duplicated into the persisted result so the
                    // tool trace can render a preview later. Storage truncation
                    // skips this tool (see truncate_tool_result_for_storage).
                    response: json!({
                        "status": "success",
                        "message": "Screen captured. The image is attached as visual context for this turn.",
                        "image_data": b64.clone(),
                    }),
                    attachment: Some(ToolAttachment {
                        mime_type: "image/png".into(),
                        data: b64,
                    }),
                },
                Err(e) => ToolResult::ok(json!({ "error": format!("Failed to capture screen: {}", e) })),
            }
        }
        "retrieve_past_memories" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let filter = filter_from_args(args);
            
            if query.is_empty() && filter.keyword.is_none() && filter.memory_type.is_none() {
                return ToolResult::ok(json!({ "error": "At least one search parameter (query, keyword, or type) is required." }));
            }

            let api_key = {
                let connection = database.connection.lock();
                match crate::database::queries::get_api_token(&connection, "gemini") {
                    Ok(Some(enc_key)) => match crate::crypto::decrypt_token(&enc_key) {
                        Ok(key) => key,
                        Err(_) => return ToolResult::ok(json!({ "error": "Failed to decrypt Gemini API key." })),
                    },
                    _ => return ToolResult::ok(json!({ "error": "Gemini API key not found. Please add it in settings." })),
                }
            };

            crate::applog!("DEBUG: 🧠 Tool 'retrieve_past_memories' invoked (query present: {})", !query.is_empty());

            let embedding = if !query.is_empty() {
                let memory_client = crate::gemini::client::GeminiClient::new(api_key);
                match memory_client.generate_embedding(query).await {
                    Ok(emb) => Some(emb),
                    Err(e) => {
                        crate::applog!("DEBUG: 🧠 Embedding Generation Failed! Error: {:#?}", e);
                        return ToolResult::ok(json!({ "error": format!("Failed to generate embedding: {}", e) }));
                    }
                }
            } else {
                None
            };

            let connection = database.connection.lock();
            let value = match crate::memory::core::search_memories_filtered(&connection, embedding.as_deref(), filter) {
                Ok(memories) if !memories.is_empty() => {
                    crate::applog!("DEBUG: 🧠 Retrieved {} memories.", memories.len());
                    let memory_context = crate::memory::core::format_memories_for_prompt(&memories);
                    for m in &memories {
                        let _ = crate::memory::core::update_memory_access(&connection, &m.id);
                    }
                    json!({ "memories_found": memory_context })
                }
                Ok(_) => json!({ "message": "No relevant past memories found." }),
                Err(e) => json!({ "error": format!("Failed to retrieve memories: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "remember_this" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if content.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'content' is required." }));
            }
            let mem_type = crate::memory::core::MemoryType::from_str(
                args.get("type").and_then(|v| v.as_str()).unwrap_or("observation"),
            )
            .unwrap_or(crate::memory::core::MemoryType::Observation);
            let importance = args.get("importance").and_then(|v| v.as_f64()).unwrap_or(8.0).clamp(1.0, 10.0);

            let api_key = {
                let connection = database.connection.lock();
                match crate::database::queries::get_api_token(&connection, "gemini") {
                    Ok(Some(enc)) => match crate::crypto::decrypt_token(&enc) {
                        Ok(k) => k,
                        Err(_) => return ToolResult::ok(json!({ "error": "Failed to decrypt Gemini API key." })),
                    },
                    _ => return ToolResult::ok(json!({ "error": "Gemini API key not found." })),
                }
            };

            let client = crate::gemini::client::GeminiClient::new(api_key);
            let mut memory = crate::memory::extractor::create_memory(mem_type, content.clone(), importance);
            // Embed so the new memory is retrievable; store anyway if embedding fails.
            match client.generate_embedding(&content).await {
                Ok(emb) => memory.embedding = Some(emb),
                Err(e) => crate::applog!("DEBUG: 🧠 remember_this embed failed (storing without): {}", e),
            }

            let connection = database.connection.lock();
            if let Some(ref emb) = memory.embedding {
                if crate::memory::core::is_near_duplicate(&connection, emb, 0.95) {
                    return ToolResult::ok(json!({ "status": "already_known", "message": "Already remembered something very similar." }));
                }
            }
            let value = match crate::memory::core::store_memory(&connection, &memory) {
                Ok(_) => json!({ "status": "saved", "message": format!("Saved to memory: {}", content) }),
                Err(e) => json!({ "error": format!("Failed to save memory: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "search_memories" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let filter = filter_from_args(args);

            if query.is_empty() && filter.keyword.is_none() && filter.memory_type.is_none() && filter.after.is_none() && filter.before.is_none() {
                return ToolResult::ok(json!({ "error": "At least one search parameter (query, keyword, type, or date) is required." }));
            }

            let api_key = {
                let connection = database.connection.lock();
                match crate::database::queries::get_api_token(&connection, "gemini") {
                    Ok(Some(enc)) => match crate::crypto::decrypt_token(&enc) {
                        Ok(k) => k,
                        Err(_) => return ToolResult::ok(json!({ "error": "Failed to decrypt Gemini API key." })),
                    },
                    _ => return ToolResult::ok(json!({ "error": "Gemini API key not found." })),
                }
            };

            let embedding = if !query.is_empty() {
                let client = crate::gemini::client::GeminiClient::new(api_key);
                match client.generate_embedding(&query).await {
                    Ok(emb) => Some(emb),
                    Err(e) => {
                        return ToolResult::ok(json!({ "error": format!("Failed to embed query: {}", e) }));
                    }
                }
            } else {
                None
            };

            let connection = database.connection.lock();
            let value = match crate::memory::core::search_memories_filtered(&connection, embedding.as_deref(), filter) {
                Ok(memories) if !memories.is_empty() => {
                    let items: Vec<serde_json::Value> = memories.iter().map(|m| json!({
                        "id": m.id,
                        "type": m.memory_type.as_str(),
                        "importance": m.importance,
                        "content": m.content,
                        "created_at": m.created_at.to_rfc3339(),
                    })).collect();
                    json!({ "memories": items, "hint": "Pass an id to edit_memory or forget_memory to curate." })
                }
                Ok(_) => json!({ "message": "No matching memories found." }),
                Err(e) => json!({ "error": format!("Failed to search memories: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "edit_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if id.is_empty() || content.is_empty() {
                return ToolResult::ok(json!({ "error": "Both 'id' and 'content' are required. Get the id from search_memories first." }));
            }
            let api_key = {
                let connection = database.connection.lock();
                match crate::database::queries::get_api_token(&connection, "gemini") {
                    Ok(Some(enc)) => match crate::crypto::decrypt_token(&enc) {
                        Ok(k) => k,
                        Err(_) => return ToolResult::ok(json!({ "error": "Failed to decrypt Gemini API key." })),
                    },
                    _ => return ToolResult::ok(json!({ "error": "Gemini API key not found." })),
                }
            };
            // Re-embed the new text so retrieval stays accurate; update anyway if it fails.
            let embedding = crate::gemini::client::GeminiClient::new(api_key)
                .generate_embedding(&content).await.ok();
            let connection = database.connection.lock();
            let value = match crate::memory::core::update_memory_content(&connection, &id, &content, embedding.as_deref()) {
                Ok(true) => json!({ "status": "updated", "message": format!("Memory updated: {}", content) }),
                Ok(false) => json!({ "error": format!("No memory found with id '{}'. Run search_memories to get a valid id.", id) }),
                Err(e) => json!({ "error": format!("Failed to update memory: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "forget_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if id.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'id' is required. Get it from search_memories first." }));
            }
            let connection = database.connection.lock();
            let value = match crate::memory::core::delete_memory(&connection, &id) {
                Ok(true) => json!({ "status": "forgotten", "message": "Memory deleted." }),
                Ok(false) => json!({ "error": format!("No memory found with id '{}'.", id) }),
                Err(e) => json!({ "error": format!("Failed to delete memory: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "compress_memories" => {
            let ids_to_delete: Vec<String> = args
                .get("ids_to_delete")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let new_content = args.get("new_content").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

            if ids_to_delete.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'ids_to_delete' must be a non-empty array of IDs. Run search_memories first to get IDs." }));
            }
            if new_content.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'new_content' is required." }));
            }

            let api_key = {
                let connection = database.connection.lock();
                match crate::database::queries::get_api_token(&connection, "gemini") {
                    Ok(Some(enc)) => match crate::crypto::decrypt_token(&enc) {
                        Ok(k) => k,
                        Err(_) => return ToolResult::ok(json!({ "error": "Failed to decrypt Gemini API key." })),
                    },
                    _ => return ToolResult::ok(json!({ "error": "Gemini API key not found." })),
                }
            };

            let client = crate::gemini::client::GeminiClient::new(api_key);
            let mut memory = crate::memory::extractor::create_memory(
                crate::memory::core::MemoryType::Reflection,
                new_content.clone(),
                8.0
            );
            
            match client.generate_embedding(&new_content).await {
                Ok(emb) => memory.embedding = Some(emb),
                Err(e) => crate::applog!("DEBUG: 🧠 compress_memories embed failed: {}", e),
            }

            let connection = database.connection.lock();
            let mut deleted_count = 0;
            for id in &ids_to_delete {
                if let Ok(true) = crate::memory::core::delete_memory(&connection, id) {
                    deleted_count += 1;
                }
            }
            
            let value = match crate::memory::core::store_memory(&connection, &memory) {
                Ok(_) => json!({ "status": "compressed", "deleted_old_memories": deleted_count, "saved_new_memory": new_content }),
                Err(e) => json!({ "error": format!("Failed to save compressed memory: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "set_reminder" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("Reminder");
            let due_at = args.get("due_at").and_then(|v| v.as_str()).unwrap_or("");
            if let Err(e) = validate_rfc3339(due_at, "due_at") { return ToolResult::ok(e); }

            let conn = database.connection.lock();
            let value = match crate::database::queries::save_reminder(&conn, content, Some(due_at)) {
                Ok(_) => {
                    // Wake the reminder daemon so it re-scans without waiting
                    if let Some(manager) = app_handle.try_state::<crate::agent::reminders::ReminderManager>() {
                        manager.trigger_update();
                    }
                    json!({
                        "status": "ok",
                        "message": format!("Reminder set for {}", due_at)
                    })
                }
                Err(e) => json!({ "error": format!("Failed to set reminder: {}", e) }),
            };
            ToolResult::ok(value)
        }
        "search_notes" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let modified_after = args.get("modified_after").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.timestamp());

            if path.is_empty() || query.is_empty() {
                return ToolResult::ok(json!({ "error": "Path and query are required for searching." }));
            }
            // Filesystem traversal can be heavy — run off the async runtime.
            let value = tokio::task::spawn_blocking(move || {
                let mut results = Vec::new();
                for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file()
                        && entry.path().extension().is_some_and(|ext| ext == "md")
                    {
                        if let Some(min_time) = modified_after {
                            let mtime = fs::metadata(entry.path()).ok()
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            if (mtime as i64) < min_time {
                                continue;
                            }
                        }

                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            if content.to_lowercase().contains(&query) {
                                results.push(entry.path().to_string_lossy().into_owned());
                            }
                        }
                    }
                    if results.len() >= limit { break; }
                }
                json!({ "matches": results })
            })
            .await
            .unwrap_or_else(|e| json!({ "error": format!("search_notes task failed: {}", e) }));
            ToolResult::ok(value)
        }
        "search_filesystem" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let extension = args.get("file_extension").and_then(|v| v.as_str()).map(|s| s.to_lowercase());

            if path.is_empty() || query.is_empty() {
                return ToolResult::ok(json!({ "error": "Path and query required." }));
            }
            let value = tokio::task::spawn_blocking(move || {
                let mut results = Vec::new();
                for entry in WalkDir::new(&path)
                    .max_depth(max_depth)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if let Some(ref ext) = extension {
                        if entry.path().extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some(ext.clone()) {
                            continue;
                        }
                    }
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains(&query) {
                        results.push(entry.path().to_string_lossy().into_owned());
                    }
                    if results.len() >= limit { break; }
                }
                json!({ "matches": results })
            })
            .await
            .unwrap_or_else(|e| json!({ "error": format!("search_filesystem task failed: {}", e) }));
            ToolResult::ok(value)
        }
        "search_vault" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if query.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'query' is required." }));
            }
            let root = args.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
                .or_else(|| vault_root_from_db(database));
            let root = match root {
                Some(r) if !r.is_empty() => r,
                _ => return ToolResult::ok(json!({ "error": "No vault path found. Enable Obsidian in settings or pass an explicit 'path'." })),
            };
            let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
            let extension = args.get("file_extension").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
            let modified_after = args.get("modified_after").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.timestamp());
            let modified_before = args.get("modified_before").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.timestamp());

            let value = tokio::task::spawn_blocking(move || {
                let terms: Vec<String> = query.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
                // (score, match_json) — collected across the whole vault, then ranked.
                let mut scored: Vec<(usize, serde_json::Value)> = Vec::new();
                let mut files_scanned = 0usize;
                for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                    if !entry.file_type().is_file() { continue; }
                    let p = entry.path();
                    
                    if let Some(ref ext) = extension {
                        if p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some(ext.clone()) {
                            continue;
                        }
                    } else if p.extension().and_then(|e| e.to_str()) != Some("md") { 
                        continue; 
                    }

                    if is_hidden_path(p) { continue; }
                    
                    if let Ok(meta) = fs::metadata(p) { 
                        if meta.len() > 1_000_000 { continue; } 
                        if let Some(min_time) = modified_after {
                            let mtime = meta.modified().ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            if (mtime as i64) < min_time { continue; }
                        }
                        if let Some(max_time) = modified_before {
                            let mtime = meta.modified().ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            if (mtime as i64) > max_time { continue; }
                        }
                    }

                    let content = match fs::read_to_string(p) { Ok(c) => c, Err(_) => continue };
                    files_scanned += 1;
                    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    let fname_bonus = terms.iter().filter(|t| fname.contains(t.as_str())).count();
                    for (i, line) in content.lines().enumerate() {
                        let ll = line.to_lowercase();
                        let hits = terms.iter().filter(|t| ll.contains(t.as_str())).count();
                        if hits == 0 { continue; }
                        let score = hits + fname_bonus;
                        let snippet: String = line.trim().chars().take(200).collect();
                        scored.push((score, json!({
                            "file": p.to_string_lossy(),
                            "line": i + 1,
                            "snippet": snippet,
                            "score": score
                        })));
                    }
                    if scored.len() > 3000 { break; } // safety cap before sort
                }
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                let matches: Vec<serde_json::Value> = scored.into_iter().take(max_results).map(|(_, v)| v).collect();
                json!({
                    "query": query,
                    "root": root,
                    "files_scanned": files_scanned,
                    "match_count": matches.len(),
                    "matches": matches,
                    "hint": "Call read_file_lines on a match's file around its line number to read more context."
                })
            })
            .await
            .unwrap_or_else(|e| json!({ "error": format!("search_vault task failed: {}", e) }));
            ToolResult::ok(value)
        }
        "list_recent_notes" => {
            let root = args.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
                .or_else(|| vault_root_from_db(database));
            let root = match root {
                Some(r) if !r.is_empty() => r,
                _ => return ToolResult::ok(json!({ "error": "No vault path found. Enable Obsidian in settings or pass an explicit 'path'." })),
            };
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let value = tokio::task::spawn_blocking(move || {
                let mut notes: Vec<(u64, serde_json::Value)> = Vec::new();
                for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                    if !entry.file_type().is_file() { continue; }
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
                    if is_hidden_path(p) { continue; }
                    let mtime = fs::metadata(p).ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    // Title = first non-empty line, with leading markdown heading marks stripped.
                    let title = fs::read_to_string(p).ok()
                        .and_then(|c| c.lines()
                            .map(|l| l.trim_start_matches('#').trim().to_string())
                            .find(|l| !l.is_empty()))
                        .unwrap_or_else(|| p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string());
                    let modified = chrono::DateTime::from_timestamp(mtime as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default();
                    notes.push((mtime, json!({
                        "file": p.to_string_lossy(),
                        "title": title.chars().take(120).collect::<String>(),
                        "modified": modified
                    })));
                }
                notes.sort_by(|a, b| b.0.cmp(&a.0));
                let recent: Vec<serde_json::Value> = notes.into_iter().take(limit).map(|(_, v)| v).collect();
                json!({ "count": recent.len(), "notes": recent })
            })
            .await
            .unwrap_or_else(|e| json!({ "error": format!("list_recent_notes task failed: {}", e) }));
            ToolResult::ok(value)
        }
        "search_by_tag" => {
            let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("")
                .trim().trim_start_matches('#').to_lowercase();
            if tag.is_empty() {
                return ToolResult::ok(json!({ "error": "Field 'tag' is required." }));
            }
            let root = args.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
                .or_else(|| vault_root_from_db(database));
            let root = match root {
                Some(r) if !r.is_empty() => r,
                _ => return ToolResult::ok(json!({ "error": "No vault path found. Enable Obsidian in settings or pass an explicit 'path'." })),
            };

            let value = tokio::task::spawn_blocking(move || {
                let inline = format!("#{}", tag);
                let mut results: Vec<String> = Vec::new();
                for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                    if !entry.file_type().is_file() { continue; }
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
                    if is_hidden_path(p) { continue; }
                    let content = match fs::read_to_string(p) { Ok(c) => c.to_lowercase(), Err(_) => continue };

                    let has_inline = content.contains(&inline);
                    // Frontmatter `tags:` block — handles both `tags: [a, b]` and the
                    // multi-line `tags:\n  - a\n  - b` form. Only scans the top of the file.
                    let mut has_frontmatter = false;
                    let mut in_tags = false;
                    for l in content.lines().take(30) {
                        let lt = l.trim_start();
                        if lt.starts_with("tags:") {
                            if lt.contains(&tag) { has_frontmatter = true; break; }
                            in_tags = true;
                            continue;
                        }
                        if in_tags {
                            if lt.starts_with('-') {
                                if lt.contains(&tag) { has_frontmatter = true; break; }
                            } else if !lt.is_empty() && !l.starts_with(' ') {
                                in_tags = false; // a new top-level key ends the tags block
                            }
                        }
                    }

                    if has_inline || has_frontmatter {
                        results.push(p.to_string_lossy().into_owned());
                    }
                    if results.len() >= 50 { break; }
                }
                json!({ "tag": tag, "match_count": results.len(), "files": results })
            })
            .await
            .unwrap_or_else(|e| json!({ "error": format!("search_by_tag task failed: {}", e) }));
            ToolResult::ok(value)
        }
        _ => ToolResult::ok(json!({ "error": format!("Unknown asynchronous tool: {}", name) })),
    }
}

// =============================================================================
// Standalone weather fetch — used by dashboard.rs too
// =============================================================================

pub async fn fetch_weather(location: &str) -> serde_json::Value {
    let url = format!("https://wttr.in/{}?format=j1", location);

    static WEATHER_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    let client = WEATHER_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    });

    match client.get(&url).send().await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(current) = data
                    .get("current_condition")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                {
                    let temp = current.get("temp_C").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let desc = current
                        .get("weatherDesc")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let humidity = current.get("humidity").and_then(|v| v.as_str()).unwrap_or("unknown");

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
