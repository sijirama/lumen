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
use tauri::State;

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

    //INFO: 1. Get Conversation History (Sliding Window: last 10 messages)
    let history = {
        let connection = database.connection.lock();
        get_chat_messages(&connection, request.session_id.as_deref(), 10)
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
    let mut parts = vec![crate::gemini::client::GeminiPart {
        text: Some(request.message.clone()),
        function_call: None,
        function_response: None,
        inline_data: None,
    }];

    if let Some(ref b64) = request.base64_image {
        parts.push(crate::gemini::client::GeminiPart {
            text: None,
            function_call: None,
            function_response: None,
            inline_data: Some(crate::gemini::client::InlineData {
                mime_type: "image/png".to_string(),
                data: b64.clone(),
            }),
        });
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
    let client = GeminiClient::new(api_key);

    //INFO: Enhance system instruction with specific user info
    let mut system_instruction = get_default_system_instruction();

    if let Some(ctx) = context {
        system_instruction.push_str("\n\n--- DYNAMIC KNOWLEDGE (BACKGROUND ONLY) ---");
        system_instruction.push_str(
            "\nThis is the user's current digital state. DO NOT acknowledgment it unless relevant.",
        );
        system_instruction.push_str("\nIf the user says 'hi' or chats, respond socially. DO NOT mention tasks unless they ask.");
        system_instruction.push_str(&format!("\n\n{}", ctx));
        system_instruction.push_str("\n-------------------------------------------");
    }

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

    system_instruction.push_str("\n\n🎯 CONVERSATIONAL RULE: If the user says 'hi', 'hello', or is just being social, respond ONLY with warmth and conversation. DO NOT mention tasks, technical context, or potential actions unless the user initiates it. Be a friend first, a sidekick second.");

    let mut current_messages = gemini_messages;
    let mut final_response_text = String::new();

    let mut tools_were_called = false;

    //INFO: Tool execution loop (max 5 turns to prevent infinite loops)
    for _ in 0..5 {
        let response_parts = client
            .send_chat(
                current_messages.clone(),
                Some(&system_instruction),
                Some(tools.clone()),
            )
            .await
            .map_err(|e| format!("Failed to get AI response: {}", e))?;

        //INFO: Record the model's response in history for the next loop turn
        let mut clean_response_parts = Vec::new();
        for part in &response_parts {
            if let Some(text) = &part.text {
                if !final_response_text.contains(text) || part.function_call.is_none() {
                    clean_response_parts.push(part.clone());
                }
            } else {
                clean_response_parts.push(part.clone());
            }
        }

        current_messages.push(crate::gemini::client::GeminiContent {
            role: Some("model".to_string()),
            parts: clean_response_parts,
        });

        let mut has_function_calls = false;
        let mut function_responses = Vec::new();

        for part in response_parts {
            if let Some(text) = part.text {
                if !final_response_text.ends_with(&text) {
                    let _ = app_handle.emit("assistant-reply-turn", text.clone());

                    if !final_response_text.is_empty() {
                        final_response_text.push_str("\n\n");
                    }
                    final_response_text.push_str(&text);
                }
            }
            if let Some(call) = part.function_call {
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
                {
                    let result =
                        crate::gemini::tools::execute_tool_async(&call.name, &call.args, &database)
                            .await;

                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name, result,
                    ));
                } else {
                    let result = {
                        let connection = database.connection.lock();
                        crate::gemini::tools::execute_tool_sync(
                            &call.name,
                            &call.args,
                            obsidian_config.as_ref(),
                            &connection,
                        )
                    };
                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name, result,
                    ));
                }
            }
        }

        if has_function_calls {
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
                        crate::gemini::client::GeminiPart {
                            text: Some("[VISUAL CONTEXT ATTACHED]".to_string()),
                            function_call: None,
                            function_response: None,
                            inline_data: None,
                        },
                        crate::gemini::client::GeminiPart {
                            text: None,
                            function_call: None,
                            function_response: None,
                            inline_data: Some(crate::gemini::client::InlineData {
                                mime_type: "image/png".to_string(),
                                data: b64,
                            }),
                        },
                    ],
                });
            }
            continue;
        } else {
            break;
        }
    }

    if final_response_text.is_empty() {
        if tools_were_called {
            final_response_text = "Done! ✨".to_string();
        } else {
            return Err("AI failed to provide a text response after tool execution".to_string());
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

    let assistant_message = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: final_response_text.clone(),
        image_data: None, // Assistant doesn't send images back currently
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
            content: final_response_text,
            image_data: None,
            created_at: assistant_message.created_at,
        },
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

//INFO: Builds context string from integrations (calendar, notes, etc.)
fn build_chat_context(database: &State<Database>) -> Result<Option<String>, String> {
    let connection = database.connection.lock();
    let mut context_parts: Vec<String> = Vec::new();

    //INFO: Get today's date info
    let today = Local::now();
    let today_str = today.format("%A, %b %d").to_string();
    let current_time = today.format("%H:%M").to_string();
    let iso_now = today.to_rfc3339();

    context_parts.push(format!("Today: {} at {}", today_str, current_time));

    //INFO: Add user profile info
    if let Ok(Some(profile)) = get_user_profile(&connection) {
        context_parts.push(format!("User Name: {}", profile.display_name));
    }

    context_parts.push(format!("\n[TECHNICAL CONTEXT]\nISO_NOW: {}", iso_now));

    //INFO: Integration Status (Helpful for AI to know what's possible)
    let mut status_parts = Vec::new();
    status_parts.push("--- INTEGRATION STATUS ---".to_string());

    let g_int = get_integration(&connection, "google").ok().flatten();
    status_parts.push(format!(
        "Google Services: {}",
        if g_int.map_or(false, |i| i.enabled) {
            "ENABLED"
        } else {
            "DISABLED"
        }
    ));

    let o_int = get_integration(&connection, "obsidian").ok().flatten();
    status_parts.push(format!(
        "Obsidian: {}",
        if o_int.map_or(false, |i| i.enabled) {
            "ENABLED"
        } else {
            "DISABLED"
        }
    ));

    status_parts.push("--------------------------".to_string());
    context_parts.push(status_parts.join("\n"));

    //INFO: Add calendar events if integration is enabled
    let google_integration = get_integration(&connection, "google")
        .map_err(|e| format!("Failed to check Google integration: {}", e))?;

    if let Some(integration) = google_integration {
        if integration.enabled {
            //INFO: Get today's events
            let start_of_day = today.format("%Y-%m-%dT00:00:00").to_string();
            let end_of_day = today.format("%Y-%m-%dT23:59:59").to_string();

            if let Ok(events) = get_calendar_events(&connection, &start_of_day, &end_of_day) {
                if !events.is_empty() {
                    let mut events_str = String::from("Today's calendar events:\n");
                    for event in events {
                        events_str
                            .push_str(&format!("- {} at {}\n", event.title, event.start_time));
                    }
                    context_parts.push(events_str);
                }
            }
        }
    }

    //INFO: Add Obsidian notes if integration is enabled
    let obsidian_integration = get_integration(&connection, "obsidian")
        .map_err(|e| format!("Failed to check Obsidian integration: {}", e))?;

    if let Some(integration) = obsidian_integration {
        if integration.enabled {
            if let Some(config) = integration.config {
                //INFO: Try to read today's daily note
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                    if let Some(vault_path) = config_json.get("vault_path").and_then(|v| v.as_str())
                    {
                        let daily_notes_folder = config_json
                            .get("daily_notes_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let date_format_raw = config_json
                            .get("daily_notes_format")
                            .and_then(|v| v.as_str())
                            .unwrap_or("YYYY-MM-DD");

                        // Convert Obsidian/Moment format to Chrono format
                        let chrono_format = date_format_raw
                            .replace("YYYY", "%Y")
                            .replace("MM", "%m")
                            .replace("DD", "%d");

                        let daily_note_name = format!("{}.md", today.format(&chrono_format));
                        let daily_note_path = std::path::Path::new(vault_path)
                            .join(daily_notes_folder)
                            .join(&daily_note_name);

                        if daily_note_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&daily_note_path) {
                                //INFO: Truncate if too long
                                let truncated_content = if content.len() > 2000 {
                                    format!("{}... (truncated)", &content[..2000])
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
