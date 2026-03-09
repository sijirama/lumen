use crate::database::{queries, Database};
use arboard::Clipboard;
use clipboard_master::{CallbackResult, ClipboardHandler, Master};

struct Handler {
    database: Database,
    last_content: String,
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        // Create a temporary clipboard handle to read the content
        // This avoids ownership/Send issues with keeping a persistent Clipboard handle
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() && trimmed != self.last_content {
                    println!("📋 Clipboard Manager: Event received! Surgical capture initiated ({} chars)", trimmed.len());

                    let connection = self.database.connection.lock();
                    if let Err(e) = queries::save_clipboard_item(&connection, trimmed, "text") {
                        eprintln!("❌ Clipboard Manager: Failed to save to vault: {}", e);
                    }

                    self.last_content = trimmed.to_string();

                    // 🧠 Latent Memory Extraction Hook (mod-5 threshold for testing)
                    const CLIPBOARD_EXTRACTION_THRESHOLD: i64 = 5;
                    if let Ok(count) = queries::count_clipboard_items(&connection) {
                        println!("DEBUG: 🧠 PULSE: Clipboard history count: {}. (Threshold: {})", count, CLIPBOARD_EXTRACTION_THRESHOLD);
                        if count > 0 && count % CLIPBOARD_EXTRACTION_THRESHOLD == 0 {
                            println!("DEBUG: 🧠 TRIGGER: Clipboard memory extraction triggered! Initializing background task...");
                            
                            let db_clone = self.database.clone();
                            // Grab last 10 items for batching
                            let recent_items = queries::get_recent_clipboard_items(&connection, 10).unwrap_or_default();
                            let items_text: Vec<String> = recent_items.into_iter().map(|i| i.content).collect();

                            tokio::spawn(async move {
                                // Fetch API Key
                                let api_key = {
                                    let conn = db_clone.connection.lock();
                                    queries::get_api_token(&conn, "gemini").ok().flatten()
                                };

                                if let Some(encrypted) = api_key {
                                    if let Ok(key) = crate::crypto::decrypt_token(&encrypted) {
                                        let client = crate::gemini::client::GeminiClient::new(key);
                                        let user_name = {
                                            let conn = db_clone.connection.lock();
                                            queries::get_user_profile(&conn)
                                                .ok()
                                                .flatten()
                                                .map(|p| p.display_name)
                                                .unwrap_or_else(|| "User".to_string())
                                        };
                                        let prompt = crate::memory::extractor::build_clipboard_extraction_prompt(&items_text, &user_name);
                                        
                                        println!("DEBUG: 🧠 Processing clipboard memories via Gemini...");
                                        let result = client.send_chat(
                                            vec![crate::gemini::client::GeminiContent {
                                                role: Some("user".to_string()),
                                                parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                                            }],
                                            Some("You are a memory agent. Return ONLY valid JSON arrays."),
                                            None,
                                            Some(crate::gemini::client::GenerationConfig {
                                                response_mime_type: Some("application/json".to_string()),
                                                response_schema: None,
                                            }),
                                        ).await;

                                        if let Ok(resp) = result {
                                            let text = resp.parts.iter().filter_map(|p| p.text.as_ref()).cloned().collect::<Vec<_>>().join("");
                                            if let Ok(mut memories) = crate::memory::extractor::parse_extracted_memories(&text) {
                                                println!("DEBUG: 🧠 Extracted {} memories from clipboard!", memories.len());
                                                for memory in &mut memories {
                                                    // Embed and Store
                                                    if let Ok(emb) = client.generate_embedding(&memory.content).await {
                                                        memory.embedding = Some(emb);
                                                        let conn = db_clone.connection.lock();
                                                        let _ = crate::memory::core::store_memory(&conn, memory);
                                                        let memory_snippet = memory.content.chars().take(60).collect::<String>();
                                                        println!("DEBUG: 🧠 Stored clipboard memory: {}", memory_snippet);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: std::io::Error) -> CallbackResult {
        eprintln!("❌ Clipboard Manager: Listener error: {}", error);
        CallbackResult::Next
    }
}

pub async fn start_clipboard_manager(database: Database) {
    println!("📋 Clipboard Manager: Switched to event-driven mode. No polling, just vibes. ✨");

    let handler = Handler {
        database,
        last_content: String::new(),
    };

    // Master::run is a blocking loop, so we move it to a dedicated background thread
    std::thread::spawn(move || match Master::new(handler) {
        Ok(mut master) => {
            if let Err(e) = master.run() {
                eprintln!("❌ Clipboard Manager: Fatal listener error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("❌ Clipboard Manager: Failed to initialize listener: {}", e);
        }
    });
}
