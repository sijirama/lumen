//INFO: Chat commands for Lumen
//NOTE: Handles AI chat functionality with Gemini

use crate::crypto::decrypt_token;
use crate::database::queries::{
    clear_chat_messages, get_api_token, get_calendar_events, get_chat_messages, get_integration,
    get_user_profile, save_chat_message, ChatMessage,
};
use crate::database::Database;
use crate::gemini::{client::get_default_system_instruction, GeminiClient};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::State;

static CHAT_RESPONSE_SCHEMA: OnceLock<serde_json::Value> = OnceLock::new();

fn get_chat_response_schema() -> &'static serde_json::Value {
    CHAT_RESPONSE_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "type": "object",
            "properties": {
                "response": {
                    "type": "string",
                    "description": "The conversational reply to the user. Use markdown for formatting."
                },
                "suggestedView": {
                    "type": "string",
                    "enum": ["chat", "calendar"],
                    "description": "The view to transition to. Use 'calendar' if the user is asking about their schedule."
                },
                "suggestedDate": {
                    "type": "string",
                    "description": "The specific ISO-8601 date to show in the calendar (e.g., '2024-03-25'). Use only if transitioning to calendar."
                }
            },
            "required": ["response", "suggestedView"]
        })
    })
}

//INFO: Chat message for frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessageResponse {
    pub id: Option<i64>,
    pub role: String,
    pub content: String,
    pub image_data: Option<String>,
    pub created_at: String,
}


//INFO: Request to send a chat message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
    pub session_id: Option<String>,
    pub base64_image: Option<String>,
}

//INFO: Response from sending a chat message
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub user_message: ChatMessageResponse,
    pub assistant_message: ChatMessageResponse,
    pub suggested_view: Option<String>,
    pub suggested_date: Option<String>, // ISO date string for calendar view
}

//INFO: Sends a message to the AI and returns the response
#[tauri::command]
pub async fn send_chat_message(
    app_handle: tauri::AppHandle,
    database: State<'_, Database>,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
    use tauri::Emitter;

    //INFO: Get the Gemini API key from the database
    let api_key = {
        let connection = database.connection.lock();
        let encrypted_key = get_api_token(&connection, "gemini")
            .map_err(|e| format!("Failed to get API key: {}", e))?
            .ok_or_else(|| {
                "Gemini API key not configured. Please add your API key in Settings.".to_string()
            })?;

        decrypt_token(&encrypted_key).map_err(|e| format!("Failed to decrypt API key: {}", e))?
    };

    //INFO: 1. Get Conversation History (Sliding Window: last 50 messages)
    let history = {
        let connection = database.connection.lock();
        get_chat_messages(&connection, request.session_id.as_deref(), 20)
            .map_err(|e| format!("Failed to get history: {}", e))?
    };

    //INFO: 2. Build context from integrations
    let context = build_chat_context(&database)?;

    //INFO: 3. Convert history to Gemini format (History is already chronological)
    let mut gemini_messages = Vec::new();
    for msg in history {
        gemini_messages.push(crate::gemini::client::GeminiContent {
            role: Some(if msg.role == "user" {
                "user".to_string()
            } else {
                "model".to_string()
            }),
            parts: vec![crate::gemini::client::GeminiPart::text(msg.content)],
        });
    }

    //INFO: 4. Add current message
    let mut parts = vec![crate::gemini::client::GeminiPart::text(request.message.clone())];

    if let Some(ref b64) = request.base64_image {
        parts.push(crate::gemini::client::GeminiPart::inline_data(
            "image/png".to_string(),
            b64.clone(),
        ));
    }

    gemini_messages.push(crate::gemini::client::GeminiContent {
        role: Some("user".to_string()),
        parts,
    });

    //INFO: 5. Load Tools
    let tools = crate::gemini::tools::get_tool_declarations();

    let obsidian_config = {
        let connection = database.connection.lock();
        get_integration(&connection, "obsidian")
            .ok()
            .flatten()
            .and_then(|i| i.config)
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
    };

    //INFO: 6. Send to Gemini (with Tool Loop)
    let client = GeminiClient::new(api_key.clone());

    //INFO: Enhance system instruction with specific user info
    let mut system_instruction = get_default_system_instruction();

    if let Some(ctx) = context {
        system_instruction.push_str("\n\n--- CURRENT DIGITAL STATE (BACKGROUND CONTEXT) ---");
        system_instruction.push_str(
            "\nThis is the user's active screen/system state. Use it ONLY if relevant to their request.",
        );
        system_instruction.push_str("\nIf the user says 'hi' or chats, respond socially. DO NOT mention system details unless asked.");
        system_instruction.push_str(&format!("\n\n{}", ctx));
        system_instruction.push_str("\n-------------------------------------------");
    }

    //INFO: 6.5 Memory retrieval is now handled explicitly via the retrieve_past_memories tool.

    if let Some(config) = &obsidian_config {
        system_instruction.push_str("\n\n--- OBSIDIAN CONFIGURATION ---");
        if let Some(path) = config.get("vault_path").and_then(|v| v.as_str()) {
            system_instruction.push_str(&format!("\nVault path: {}", path));
        }
        if let Some(folder) = config.get("daily_notes_path").and_then(|v| v.as_str()) {
            if !folder.is_empty() {
                system_instruction.push_str(&format!(
                    "\nDaily Notes folder (relative to vault): {}",
                    folder
                ));
            }
        }
        if let Some(format) = config.get("daily_notes_format").and_then(|v| v.as_str()) {
            system_instruction.push_str(&format!(
                "\nDaily Notes date format (Moment.js syntax): {}",
                format
            ));
        }
        system_instruction.push_str("\n------------------------------");
    }

    system_instruction.push_str("\n\n🎯 CONVERSATIONAL RULES:\n1. If the user says 'hi', 'hello', 'hey', 'what's up', or is just being social, respond IMMEDIATELY with warmth in the 'response' field. Do NOT call any tools. Do NOT retrieve memories. Just be friendly.\n2. Only use tools when the user asks a SPECIFIC question that requires data (calendar, weather, files, etc.).\n3. The 'response' field is MANDATORY in every reply. Never skip it.");

    let mut current_messages = gemini_messages;
    let mut final_response_text = String::new();

    let mut tools_were_called = false;

    //INFO: Tool execution loop — uses non-streaming for tool rounds
    //NOTE: Only the FINAL response (no function calls) gets streamed to the UI
    let config = crate::gemini::client::GenerationConfig {
        response_mime_type: Some("application/json".to_string()),
        response_schema: Some(get_chat_response_schema().clone()),
    };

    //INFO: Per-tool call counter to prevent runaway tool loops
    let mut tool_call_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    const MAX_CALLS_PER_TOOL: usize = 2;
    const MAX_TOOL_ROUNDS: usize = 5;

    for _i in 0..MAX_TOOL_ROUNDS {
        // Use non-streaming send_chat for tool execution rounds
        let chat_response = client
            .send_chat(
                current_messages.clone(),
                Some(&system_instruction),
                Some(tools.clone()),
                Some(config.clone()),
            )
            .await
            .map_err(|e| format!("Failed to get AI response: {}", e))?;

        let response_parts = chat_response.parts;

        // Record the model's response in history
        current_messages.push(crate::gemini::client::GeminiContent {
            role: Some("model".to_string()),
            parts: response_parts.clone(),
        });

        let mut has_function_calls = false;
        let mut function_responses = Vec::new();

        for part in &response_parts {
            // Extract text from non-tool-call responses
            if let Some(text) = &part.text {
                // Reset final text each round — only the last round's text matters
                final_response_text.clear();
                final_response_text.push_str(text);

                // Emit to frontend for real-time display
                let mut emit_text = text.clone();
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(resp) = json_val.get("response").and_then(|v| v.as_str()) {
                        emit_text = resp.to_string();
                    }
                }
                let _ = app_handle.emit("assistant-reply-turn", emit_text);
            }

            if let Some(call) = &part.function_call {
                // Check if this tool has been called too many times
                let count = tool_call_counts.entry(call.name.clone()).or_insert(0);
                *count += 1;

                if *count > MAX_CALLS_PER_TOOL {
                    println!("DEBUG: ⚠️ Tool '{}' hit call limit ({}), skipping.", call.name, MAX_CALLS_PER_TOOL);
                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name.clone(),
                        serde_json::json!({ "error": format!("Tool '{}' has already been called {} times this turn. Please provide your response now using the information you already have.", call.name, MAX_CALLS_PER_TOOL) }),
                    ));
                } else {

                has_function_calls = true;
                tools_were_called = true;
                if call.name == "get_weather"
                    || call.name == "get_google_calendar_events"
                    || call.name == "get_unread_emails"
                    || call.name == "send_email"
                    || call.name == "create_calendar_event"
                    || call.name == "list_google_tasks"
                    || call.name == "create_google_task"
                    || call.name == "take_screenshot"
                    || call.name == "retrieve_past_memories"
                {
                    let res =
                        crate::gemini::tools::execute_tool_async(&call.name, &call.args, &database)
                            .await;

                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name.clone(),
                        res,
                    ));
                } else {
                    let res = {
                        let connection = database.connection.lock();
                        crate::gemini::tools::execute_tool_sync(
                            &call.name,
                            &call.args,
                            obsidian_config.as_ref(),
                            &connection,
                        )
                    };
                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name.clone(),
                        res,
                    ));
                }
                } // Close the newly added else block
            }
        }

        if has_function_calls {
            // Clear the streaming bubble so it doesn't show stale tool-call text
            let _ = app_handle.emit("assistant-reply-clear", ());

            let mut screenshot_data = None;
            for resp in &mut function_responses {
                if let Some(f_resp) = &mut resp.function_response {
                    if f_resp.name == "take_screenshot" {
                        if let Some(obj) = f_resp.response.as_object_mut() {
                            if let Some(b64) = obj
                                .get("image_data")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                            {
                                screenshot_data = Some(b64);
                                obj.remove("image_data");
                                obj.insert("info".into(), serde_json::json!("Screenshot captured successfully. You can now see the image in this Turn."));
                            }
                        }
                    }
                }
            }

            current_messages.push(crate::gemini::client::GeminiContent {
                role: Some("function".to_string()),
                parts: function_responses,
            });
            if let Some(b64) = screenshot_data {
                current_messages.push(crate::gemini::client::GeminiContent {
                    role: Some("user".to_string()),
                    parts: vec![
                        crate::gemini::client::GeminiPart::text("[VISUAL CONTEXT ATTACHED]".to_string()),
                        crate::gemini::client::GeminiPart::inline_data(
                            "image/png".to_string(),
                            b64,
                        ),
                    ],
                });
            }
            continue;
        } else {
            break;
        }
    }

    //INFO: Safety net — if the model used tools but never produced text,
    //      force one last call WITHOUT tools so it MUST reply with text.
    if final_response_text.is_empty() {
        println!("DEBUG: ⚠️ No text after tool loop. Forcing a final text-only call...");

        let forced_response = client
            .send_chat(
                current_messages.clone(),
                Some(&system_instruction),
                None, // No tools — forces a pure text response
                Some(config.clone()),
            )
            .await
            .map_err(|e| format!("Failed to get forced response: {}", e))?;

        for part in &forced_response.parts {
            if let Some(text) = &part.text {
                final_response_text = text.clone();

                // Emit to frontend
                let mut emit_text = text.clone();
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(resp) = json_val.get("response").and_then(|v| v.as_str()) {
                        emit_text = resp.to_string();
                    }
                }
                let _ = app_handle.emit("assistant-reply-turn", emit_text);
            }
        }

        if final_response_text.is_empty() {
            return Err("Lumen processed the request but couldn't generate a response. Please try again.".to_string());
        }
    }

    //INFO: Save both messages to the database
    let now = chrono::Utc::now().to_rfc3339();

    let user_message = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: request.message.clone(),
        image_data: request.base64_image.clone(),
        created_at: now.clone(),
        session_id: request.session_id.clone(),
    };

    //INFO: Parse Structured JSON output BEFORE saving to DB
    let mut actual_final_text = final_response_text.clone();
    let mut suggested_view = None;
    let mut suggested_date = None;

    // We might have multiple chunks in final_response_text separated by \n\n
    // Find the last valid JSON chunk
    for text_chunk in final_response_text.rsplit("\n\n") {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text_chunk) {
            if let Some(resp) = json_val.get("response").and_then(|v| v.as_str()) {
                actual_final_text = resp.to_string();
            }
            if let Some(view) = json_val.get("suggestedView").and_then(|v| v.as_str()) {
                suggested_view = Some(view.to_string());
            }
            if let Some(date) = json_val.get("suggestedDate").and_then(|v| v.as_str()) {
                suggested_date = Some(date.to_string());
            }
            break;
        }
    }

    let assistant_message = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: actual_final_text.clone(),
        image_data: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        session_id: request.session_id,
    };

    //INFO: Save messages to database
    let (user_id, assistant_id) = {
        let connection = database.connection.lock();
        let user_id = save_chat_message(&connection, &user_message)
            .map_err(|e| format!("Failed to save user message: {}", e))?;
        let assistant_id = save_chat_message(&connection, &assistant_message)
            .map_err(|e| format!("Failed to save assistant message: {}", e))?;
        (user_id, assistant_id)
    };

    //INFO: Latent Memory Extraction Trigger (mod-based)
    const MEMORY_EXTRACTION_THRESHOLD: i64 = 50;
    {
        let connection = database.connection.lock();
        if let Ok(total_count) = crate::database::queries::count_chat_messages(&connection) {
            println!("DEBUG: 🧠 PULSE: Current chat message count: {}. (Threshold: {})", total_count, MEMORY_EXTRACTION_THRESHOLD);
            if total_count > 0 && total_count % MEMORY_EXTRACTION_THRESHOLD == 0 {
                println!("DEBUG: 🧠 TRIGGER: Memory extraction threshold hit! Initializing background task...");
                
                // Grab the last N messages for extraction
                let mut recent_messages = crate::database::queries::get_chat_messages(
                    &connection, 
                    None, 
                    MEMORY_EXTRACTION_THRESHOLD as i32,
                ).unwrap_or_default();
                
                // Reverse to chronological order (get_chat_messages returns newest first)
                recent_messages.reverse();

                let messages_for_extraction: Vec<String> = recent_messages
                    .iter()
                    .map(|m| format!("{}: {}", if m.role == "user" { "Sijibomi" } else { "Lumen" }, m.content))
                    .collect();

                // Clone what we need for the background task
                let db_clone = database.inner().clone();
                let api_key_clone = api_key.clone();
                // Fire and forget - async background extraction
                tokio::spawn(async move {
                    println!("DEBUG: 🧠 Starting background memory extraction...");

                    let user_name = {
                        let conn = db_clone.connection.lock();
                        crate::database::queries::get_user_profile(&conn)
                            .ok()
                            .flatten()
                            .map(|p| p.display_name)
                            .unwrap_or_else(|| "User".to_string())
                    };

                    let prompt = crate::memory::extractor::build_chat_extraction_prompt(&messages_for_extraction, &user_name);
                    let client = GeminiClient::new(api_key_clone.clone());

                    // Ask Gemini to extract memories
                    let extraction_result = client.send_chat(
                        vec![crate::gemini::client::GeminiContent {
                            role: Some("user".to_string()),
                            parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                        }],
                        Some("You are a memory extraction agent. Return ONLY valid JSON arrays."),
                        None,
                        Some(crate::gemini::client::GenerationConfig {
                            response_mime_type: Some("application/json".to_string()),
                            response_schema: None,
                        }),
                    ).await;

                    match extraction_result {
                        Ok(chat_response) => {
                            if let Some(usage) = &chat_response.usage {
                                println!("DEBUG: 🧠 Extraction Token Usage -> Prompt: {}, Candidates: {}, Total: {}", usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count);
                            }
                            let response_text = chat_response.parts.iter()
                                .filter_map(|p| p.text.as_ref())
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("");

                            match crate::memory::extractor::parse_extracted_memories(&response_text) {
                                Ok(mut memories) => {
                                    println!("DEBUG: 🧠 Extracted {} memories from chat!", memories.len());
                                    for memory in &mut memories {
                                        // Generate embedding for each memory
                                        match client.generate_embedding(&memory.content).await {
                                            Ok(embedding) => {
                                                memory.embedding = Some(embedding);
                                                println!("DEBUG: 🧠 [{}] (importance: {}) {}", 
                                                    memory.memory_type.as_str(),
                                                    memory.importance,
                                                    memory.content.chars().take(80).collect::<String>()
                                                );
                                            }
                                            Err(e) => {
                                                println!("DEBUG: 🧠 Failed to embed memory: {}", e);
                                            }
                                        }
                                        
                                        // Store in DB
                                        let conn = db_clone.connection.lock();
                                        if let Err(e) = crate::memory::core::store_memory(&conn, memory) {
                                            println!("DEBUG: 🧠 Failed to store memory: {}", e);
                                        }
                                    }
                                    println!("DEBUG: 🧠 Memory extraction complete! ✅");

                                    // Check if we should trigger a Reflection loop
                                    {
                                        let conn = db_clone.connection.lock();
                                        match crate::memory::core::should_trigger_reflection(&conn) {
                                            Ok(true) => {
                                                println!("DEBUG: 🧠 Reflection threshold hit! Starting synthesis...");
                                                // Get the last 30 observations for reflection
                                                if let Ok(recent_obs) = crate::memory::core::get_recent_memories_by_type(
                                                    &conn,
                                                    &crate::memory::core::MemoryType::Observation,
                                                    MEMORY_EXTRACTION_THRESHOLD as usize
                                                ) {
                                                    let obs_texts: Vec<String> = recent_obs.iter().map(|o| o.content.clone()).collect();
                                                    let user_name = {
                                                        let conn = db_clone.connection.lock();
                                                        crate::database::queries::get_user_profile(&conn)
                                                            .ok()
                                                            .flatten()
                                                            .map(|p| p.display_name)
                                                            .unwrap_or_else(|| "User".to_string())
                                                    };
                                                    let prompt = crate::memory::reflection::build_reflection_prompt(&obs_texts, &user_name);
                                                    
                                                    // Drop lock for async synthesis
                                                    drop(conn);
                                                    
                                                    let api_key_reflection = api_key_clone.clone();
                                                    tokio::spawn(async move {
                                                        let client = GeminiClient::new(api_key_reflection);
                                                        println!("DEBUG: 🧠 Requesting reflection from Gemini...");
                                                        
                                                        let synthesis_result = client.send_chat(
                                                            vec![crate::gemini::client::GeminiContent {
                                                                role: Some("user".to_string()),
                                                                parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                                                            }],
                                                            Some("You are a reflection agent. Return ONLY a JSON array of reflections."),
                                                            None,
                                                            Some(crate::gemini::client::GenerationConfig {
                                                                response_mime_type: Some("application/json".to_string()),
                                                                response_schema: None,
                                                            }),
                                                        ).await;

                                                        if let Ok(resp) = synthesis_result {
                                                            let text = resp.parts.iter().filter_map(|p| p.text.as_ref()).cloned().collect::<Vec<_>>().join("");
                                                            if let Ok(reflections) = serde_json::from_str::<Vec<crate::memory::reflection::ExtractedReflection>>(&text) {
                                                                println!("DEBUG: 🧠 Synthesized {} high-level reflections!", reflections.len());
                                                                for r in reflections {
                                                                    let mut memory = crate::memory::extractor::create_memory(
                                                                        crate::memory::core::MemoryType::Reflection,
                                                                        r.content,
                                                                        r.importance
                                                                    );
                                                                    
                                                                    // Embed and store
                                                                    if let Ok(emb) = client.generate_embedding(&memory.content).await {
                                                                        memory.embedding = Some(emb);
                                                                        let conn = db_clone.connection.lock();
                                                                        let _ = crate::memory::core::store_memory(&conn, &memory);
                                                                        let reflection_snippet = memory.content.chars().take(60).collect::<String>();
                                                                        println!("DEBUG: 🧠 Stored reflection: {}", reflection_snippet);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                            Ok(false) => {}
                                            Err(e) => println!("DEBUG: 🧠 Reflection check failed: {}", e),
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("DEBUG: 🧠 Failed to parse extracted memories: {}", e);
                                    println!("DEBUG: 🧠 Raw response: {}", response_text);
                                }
                            }
                        }
                        Err(e) => {
                            println!("DEBUG: 🧠 Memory extraction LLM call failed: {}", e);
                        }
                    }
                });
            }
        }
    }

    Ok(SendMessageResponse {
        user_message: ChatMessageResponse {
            id: Some(user_id),
            role: user_message.role,
            content: user_message.content,
            image_data: user_message.image_data,
            created_at: user_message.created_at,
        },
        assistant_message: ChatMessageResponse {
            id: Some(assistant_id),
            role: assistant_message.role,
            content: actual_final_text,
            image_data: None,
            created_at: assistant_message.created_at,
        },
        suggested_view,
        suggested_date,
    })
}

//INFO: Gets chat history
#[tauri::command]
pub fn get_chat_history(
    database: State<Database>,
    session_id: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<ChatMessageResponse>, String> {
    let connection = database.connection.lock();
    let limit = limit.unwrap_or(50);

    let messages = get_chat_messages(&connection, session_id.as_deref(), limit)
        .map_err(|e| format!("Failed to get chat history: {}", e))?;

    Ok(messages
        .into_iter()
        .map(|m| ChatMessageResponse {
            id: m.id,
            role: m.role,
            content: m.content,
            image_data: m.image_data,
            created_at: m.created_at,
        })
        .collect())
}

//INFO: Clears all chat history
#[tauri::command]
pub fn clear_chat_history(database: State<Database>) -> Result<(), String> {
    let connection = database.connection.lock();

    clear_chat_messages(&connection).map_err(|e| format!("Failed to clear chat history: {}", e))
}

//INFO: Bu//INFO: Builds context string from integrations (calendar, notes, etc.)
fn build_chat_context(database: &State<Database>) -> Result<Option<String>, String> {
    let mut context_parts: Vec<String> = Vec::new();

    // 1. Static Metadata
    let today = Local::now();
    let today_str = today.format("%A, %b %d").to_string();
    let current_time = today.format("%H:%M").to_string();
    let iso_now = today.to_rfc3339();
    context_parts.push(format!("Today: {} at {}", today_str, current_time));

    // 2. Integration Data (Locked Section - Keep it brief)
    let (user_profile, g_int, o_int) = {
        let connection = database.connection.lock();
        let user_profile = get_user_profile(&connection).ok().flatten();
        let g_int = get_integration(&connection, "google").ok().flatten();
        let o_int = get_integration(&connection, "obsidian").ok().flatten();
        (user_profile, g_int, o_int)
    };

    if let Some(profile) = user_profile {
        context_parts.push(format!("User Name: {}", profile.display_name));
    }

    context_parts.push(format!("\n[TECHNICAL CONTEXT]\nISO_NOW: {}", iso_now));

    let mut status_parts = Vec::new();
    status_parts.push("--- INTEGRATION STATUS ---".to_string());
    status_parts.push(format!("Google Services: {}", if g_int.as_ref().is_some_and(|i| i.enabled) { "ENABLED" } else { "DISABLED" }));
    status_parts.push(format!("Obsidian: {}", if o_int.as_ref().is_some_and(|i| i.enabled) { "ENABLED" } else { "DISABLED" }));
    status_parts.push("--------------------------".to_string());
    context_parts.push(status_parts.join("\n"));

    // 3. Calendar Data (Locked Section)
    if let Some(integration) = g_int {
        if integration.enabled {
            let start_of_day = today.format("%Y-%m-%dT00:00:00").to_string();
            let end_of_day = today.format("%Y-%m-%dT23:59:59").to_string();
            let connection = database.connection.lock();
            if let Ok(events) = get_calendar_events(&connection, &start_of_day, &end_of_day) {
                if !events.is_empty() {
                    let mut events_str = String::from("Today's calendar events:\n");
                    for event in events {
                        events_str.push_str(&format!("- {} at {}\n", event.title, event.start_time));
                    }
                    context_parts.push(events_str);
                }
            }
        }
    }

    // 4. Obsidian Data (NO LOCKS - Pure Disk I/O)
    if let Some(integration) = o_int {
        if integration.enabled {
            if let Some(config) = integration.config {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                    if let Some(vault_path) = config_json.get("vault_path").and_then(|v| v.as_str()) {
                        let daily_notes_folder = config_json.get("daily_notes_path").and_then(|v| v.as_str()).unwrap_or("");
                        let date_format_raw = config_json.get("daily_notes_format").and_then(|v| v.as_str()).unwrap_or("YYYY-MM-DD");

                        let chrono_format = date_format_raw.replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d");
                        let daily_note_name = format!("{}.md", today.format(&chrono_format));
                        let daily_note_path = std::path::Path::new(vault_path).join(daily_notes_folder).join(&daily_note_name);

                        if daily_note_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&daily_note_path) {
                                let truncated_content = if content.chars().count() > 2000 {
                                    format!("{}... (truncated)", content.chars().take(2000).collect::<String>())
                                } else {
                                    content
                                };
                                context_parts.push(format!(
                                    "Today's daily note (NAME: {}, PATH: {}):\n{}",
                                    daily_note_name,
                                    daily_note_path.to_string_lossy(),
                                    truncated_content
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if context_parts.len() > 1 {
        Ok(Some(context_parts.join("\n\n")))
    } else {
        Ok(None)
    }
}
