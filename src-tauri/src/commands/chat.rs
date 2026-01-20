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
    pub created_at: String,
}

//INFO: Request to send a chat message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
    pub session_id: Option<String>,
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
    database: State<'_, Database>,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
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

    //INFO: 3. Convert history to Gemini format
    let mut gemini_messages = Vec::new();
    for msg in history.into_iter().rev() {
        gemini_messages.push(crate::gemini::client::GeminiContent {
            role: Some(if msg.role == "user" {
                "user".to_string()
            } else {
                "model".to_string()
            }),
            parts: vec![crate::gemini::client::GeminiPart {
                text: Some(msg.content),
                function_call: None,
                function_response: None,
            }],
        });
    }

    //INFO: 4. Add current message with context enrichment
    let user_message_content = match context {
        Some(ctx) => format!("CONTEXT:\n{}\n\nUSER MESSAGE: {}", ctx, request.message),
        None => request.message.clone(),
    };

    gemini_messages.push(crate::gemini::client::GeminiContent {
        role: Some("user".to_string()),
        parts: vec![crate::gemini::client::GeminiPart {
            text: Some(user_message_content),
            function_call: None,
            function_response: None,
        }],
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

    let mut current_messages = gemini_messages;
    let mut final_response_text = String::new();

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

        //INFO: Record the model's response in history
        current_messages.push(crate::gemini::client::GeminiContent {
            role: Some("model".to_string()),
            parts: response_parts.clone(),
        });

        let mut has_function_calls = false;
        let mut function_responses = Vec::new();

        for part in response_parts {
            if let Some(text) = part.text {
                //INFO: Append text to final response
                if !final_response_text.is_empty() {
                    final_response_text.push_str("\n\n");
                }
                final_response_text.push_str(&text);
            }
            if let Some(call) = part.function_call {
                has_function_calls = true;
                if call.name == "get_weather"
                    || call.name == "get_google_calendar_events"
                    || call.name == "get_unread_emails"
                    || call.name == "send_email"
                    || call.name == "create_calendar_event"
                    || call.name == "list_google_tasks"
                    || call.name == "create_google_task"
                {
                    let result =
                        crate::gemini::tools::execute_tool_async(&call.name, &call.args, &database)
                            .await;
                    function_responses.push(crate::gemini::client::GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(crate::gemini::client::GeminiFunctionResponse {
                            name: call.name,
                            response: result,
                        }),
                    });
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
                    function_responses.push(crate::gemini::client::GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(crate::gemini::client::GeminiFunctionResponse {
                            name: call.name,
                            response: result,
                        }),
                    });
                }
            }
        }

        if has_function_calls {
            //INFO: Push function responses to history and continue loop
            current_messages.push(crate::gemini::client::GeminiContent {
                role: Some("function".to_string()),
                parts: function_responses,
            });
            continue;
        } else {
            //INFO: No more function calls, we are done
            break;
        }
    }

    if final_response_text.is_empty() {
        return Err("AI failed to provide a text response after tool execution".to_string());
    }

    //INFO: Save both messages to the database
    let now = chrono::Utc::now().to_rfc3339();

    let user_message = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: request.message.clone(),
        created_at: now.clone(),
        session_id: request.session_id.clone(),
    };

    let assistant_message = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: final_response_text.clone(),
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
            created_at: user_message.created_at,
        },
        assistant_message: ChatMessageResponse {
            id: Some(assistant_id),
            role: assistant_message.role,
            content: final_response_text,
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
    let today_str = today.format("%A, %B %d, %Y").to_string();
    let current_time = today.format("%H:%M").to_string();

    context_parts.push(format!(
        "Current date and time: {} at {}",
        today_str, current_time
    ));

    //INFO: Add user profile info
    if let Ok(Some(profile)) = get_user_profile(&connection) {
        context_parts.push(format!("User Name: {}", profile.display_name));
    }

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
                                    "Today's daily note ({}):\n{}",
                                    daily_note_name, truncated_content
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
