//INFO: Dashboard commands for Lumen
//NOTE: Handles daily briefing summaries with hashing and AI evolution

use crate::database::{queries, Database};
use crate::gemini::client::{GeminiClient, GeminiContent, GeminiPart, GenerationConfig};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Local};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::State;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardBriefing {
    pub content: String,
    pub created_at: String,
    pub is_stale: bool,
    pub audio_data: Option<String>, // Base64 encoded audio
}

//INFO: //INFO: Gets the latest briefing from the database
#[tauri::command]
pub async fn get_dashboard_briefing(
    database: State<'_, Database>,
) -> Result<Option<DashboardBriefing>, String> {
    let latest = {
        let connection = database.connection.lock();
        queries::get_latest_briefing_summary(&connection).map_err(|e| e.to_string())?
    };

    if let Some(summary) = latest {
        let b64_audio = summary
            .audio_data
            .map(|data| general_purpose::STANDARD.encode(data));

        Ok(Some(DashboardBriefing {
            content: summary.content,
            created_at: summary.created_at,
            is_stale: false, // Inverted logic: user refreshes manually now
            audio_data: b64_audio,
        }))
    } else {
        Ok(None)
    }
}

//INFO: Generates a new briefing evolution using Gemini
#[tauri::command]
pub async fn refresh_dashboard_briefing(
    database: State<'_, Database>,
    app_handle: tauri::AppHandle,
) -> Result<DashboardBriefing, String> {
    // 1. Get user profile and API key
    let (greeting_name, api_key_encrypted) = {
        let connection = database.connection.lock();
        let profile = queries::get_user_profile(&connection).ok().flatten();
        let name = profile.as_ref().map(|p| p.display_name.clone()).unwrap_or_else(|| "User".to_string());
        
        let key = queries::get_api_token(&connection, "gemini")
            .map_err(|e| e.to_string())?
            .ok_or("Gemini API key not configured")?;
            
        (name, key)
    };

    let api_key = crate::crypto::decrypt_token(&api_key_encrypted).map_err(|e| e.to_string())?;
    let gemini_client = GeminiClient::new(api_key.clone());

    // 2. Fetch Raw Data in Parallel
    let obsidian_future = {
        let db = database.inner().clone();
        async move {
            let connection = db.connection.lock();
            let mut notes = Vec::new();
            let mut recent_files = Vec::new();

            if let Ok(Some(integration)) = queries::get_integration(&connection, "obsidian") {
                if integration.enabled {
                    if let Some(config) = integration.config {
                        if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                            if let Some(vault_path) = config_json.get("vault_path").and_then(|v| v.as_str()) {
                                let daily_notes_folder = config_json.get("daily_notes_path").and_then(|v| v.as_str()).unwrap_or("");
                                let date_format = config_json.get("daily_notes_format").and_then(|v| v.as_str()).unwrap_or("YYYY-MM-DD");
                                let chrono_format = date_format.replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d");

                                // A. Daily Notes (7 days)
                                for i in 0..7 {
                                    let target_date = Local::now() - Duration::days(i);
                                    let label = if i == 0 { "TODAY" } else if i == 1 { "YESTERDAY" } else { "PAST" };
                                    let note_name = format!("{}.md", target_date.format(&chrono_format));
                                    let note_path = Path::new(vault_path).join(daily_notes_folder).join(&note_name);

                                    if let Ok(content) = fs::read_to_string(&note_path) {
                                        notes.push(format!("### [{}] Daily Note ({})\n{}", label, target_date.format("%A, %B %d"), content));
                                    }
                                }

                                // B. Deep Vault Scan (Recently modified in last 7 days)
                                let week_ago = Local::now() - Duration::days(7);
                                let mut entries: Vec<_> = WalkDir::new(vault_path)
                                    .into_iter()
                                    .filter_map(|e| e.ok())
                                    .filter(|e| e.file_type().is_file())
                                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
                                    .filter_map(|e| {
                                        let metadata = e.metadata().ok()?;
                                        let modified: chrono::DateTime<Local> = metadata.modified().ok()?.into();
                                        if modified > week_ago {
                                            Some((e, modified))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                entries.sort_by(|a, b| b.1.cmp(&a.1));
                                if !entries.is_empty() {
                                    println!("DEBUG: Found {} recently modified Obsidian files:", entries.len());
                                }
                                for (entry, modified) in entries.into_iter().take(4) {
                                    if let Ok(content) = fs::read_to_string(entry.path()) {
                                        let file_name = entry.file_name().to_string_lossy();
                                        println!("  - [PICK] {}", file_name);
                                        // Truncate content to avoid blowing up context
                                        let snippet = if content.len() > 1000 { format!("{}...", &content[..1000]) } else { content };
                                        recent_files.push(format!("### [MODIFIED] {} (on {})\n{}", file_name, modified.format("%A, %B %d"), snippet));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let notes_str = if notes.is_empty() { "No recent Obsidian daily notes found.".to_string() } else { notes.join("\n\n") };
            let recent_str = if recent_files.is_empty() { "No other recently modified files found.".to_string() } else { recent_files.join("\n\n") };
            format!("OBSIDIAN DAILY NOTES:\n{}\n\nOTHER RECENTLY MODIFIED FILES (Last 7 Days):\n{}", notes_str, recent_str)
        }
    };

    let email_future = {
        let db = database.inner().clone();
        let api_key = api_key.clone();
        async move {
            let mut important_emails = Vec::new();
            if let Ok(has_google) = {
                let connection = db.connection.lock();
                queries::has_api_token(&connection, "google")
            } {
                if has_google {
                    let last_24h = (Local::now() - Duration::hours(24)).timestamp();
                    let query = format!("category:primary after:{}", last_24h);
                    
                    if let Ok(emails) = crate::integrations::google_gmail::fetch_recent_emails_with_query(&db, 30, Some(&query)).await {
                        if !emails.is_empty() {
                            println!("DEBUG: Found {} emails from Gmail API in last 24h:", emails.len());
                            for e in &emails {
                                println!("  - Subject: {}", e.subject.as_deref().unwrap_or("(No Subject)"));
                            }

                            let emails_json = serde_json::to_string(&emails).unwrap_or_default();
                            let filter_prompt = crate::gemini::prompt::get_email_filter_prompt(&emails_json);
                            let gemini_client = GeminiClient::new(api_key);
                            
                            match gemini_client.send_chat(
                                vec![GeminiContent {
                                    role: Some("user".to_string()),
                                    parts: vec![GeminiPart::text(filter_prompt)],
                                }],
                                Some("You are a specialized email filtering agent. Respond ONLY with valid JSON."),
                                None,
                                Some(GenerationConfig {
                                    response_mime_type: Some("application/json".to_string()),
                                    response_schema: None,
                                }),
                            ).await {
                                Ok(parts) => {
                                    let filter_response = parts.iter().filter_map(|p| p.text.as_ref()).cloned().collect::<Vec<_>>().join("");
                                    
                                    match serde_json::from_str::<Vec<crate::integrations::google_gmail::GmailMessage>>(&filter_response.trim()) {
                                        Ok(filtered) => {
                                            println!("DEBUG: Filtered down to {} important emails:", filtered.len());
                                            for f in &filtered {
                                                println!("  - [KEEP] Subject: {}", f.subject.as_deref().unwrap_or("(No Subject)"));
                                            }
                                            // Take up to 10
                                            important_emails = filtered.into_iter().take(10).collect();
                                        }
                                        Err(e) => {
                                            println!("DEBUG: Email filter JSON parse failed: {}. Falling back to top 10.", e);
                                            println!("DEBUG: Raw response was: {}", filter_response);
                                            important_emails = emails.into_iter().take(10).collect();
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("DEBUG: Email filter Gemini call failed: {}. Falling back to top 10.", e);
                                    important_emails = emails.into_iter().take(10).collect();
                                }
                            }
                        }
                    }
                }
            }
            important_emails
        }
    };

    let calendar_future = {
        let db = database.inner().clone();
        async move {
            let mut google_calendar_data = Vec::new();
            if let Ok(has_google) = {
                let connection = db.connection.lock();
                queries::has_api_token(&connection, "google")
            } {
                if has_google {
                    let start_of_search = (Local::now() - Duration::days(3)).format("%Y-%m-%dT00:00:00Z").to_string();
                    let end_of_search = (Local::now() + Duration::days(3)).format("%Y-%m-%dT23:59:59Z").to_string();

                    if let Ok(events) = crate::integrations::google_calendar::fetch_google_calendar_events(&db, &start_of_search, &end_of_search).await {
                        if !events.is_empty() {
                            println!("DEBUG: Found {} calendar events in 7-day window (3 back, 3 forward):", events.len());
                        }
                        let e_str = events.iter().map(|e| {
                            let title = e.summary.as_deref().unwrap_or("(No Title)");
                            println!("  - [KEEP] {}", title);
                            let start_str = e.start.date_time.as_deref().or(e.start.date.as_deref()).unwrap_or("unknown");
                            // Try to parse for better labeling if possible, otherwise raw
                            format!("- {} (starts at {})", title, start_str)
                        }).collect::<Vec<_>>().join("\n");
                        if !e_str.is_empty() { google_calendar_data.push(format!("Calendar Events (3 Days Backward to 3 Days Forward):\n{}", e_str)); }
                    }
                }
            }
            google_calendar_data.join("\n\n")
        }
    };

    let weather_future = async {
        match crate::gemini::tools::fetch_weather("Lagos").await {
            serde_json::Value::Object(map) => {
                format!("Weather in {}: {}°C, {}", 
                    map.get("location").and_then(|v| v.as_str()).unwrap_or("Lagos"),
                    map.get("temperature_c").and_then(|v| v.as_str()).unwrap_or("??"),
                    map.get("condition").and_then(|v| v.as_str()).unwrap_or("unknown condition")
                )
            },
            _ => "Weather data unavailable.".to_string()
        }
    };

    // Run all fetches in parallel
    let (obsidian_data, important_emails, google_calendar_data, weather_data) = tokio::join!(obsidian_future, email_future, calendar_future, weather_future);

    // 3. Construct Final Prompt and Generate Briefing
    let email_final = if important_emails.is_empty() { "No critical emails found." .to_string() } else {
        important_emails.iter().map(|m| {
            let snippet = if m.snippet.len() > 200 { format!("{}...", &m.snippet[..200]) } else { m.snippet.clone() };
            format!("- Date: {} | From: {} | Subject: {} | Snippet: {}", 
                m.date.as_deref().unwrap_or("Unknown"), 
                m.from.as_deref().unwrap_or("Unknown"), 
                m.subject.as_deref().unwrap_or("No Subject"), 
                snippet
            )
        }).collect::<Vec<_>>().join("\n")
    };
    
    let calendar_final = if google_calendar_data.is_empty() { "No upcoming calendar events." .to_string() } else { google_calendar_data };

    let now = Local::now();
    let current_time_str = now.format("%A, %B %d, %Y at %I:%M %p").to_string();

    let raw_data_context = format!(
        "CURRENT TIME: {}\n\nWEATHER:\n{}\n\nOBSIDIAN DATA:\n{}\n\nIMPORTANT EMAILS (Last 24h):\n{}\n\nCALENDAR (7-Day Window):\n{}",
        current_time_str, weather_data, obsidian_data, email_final, calendar_final
    );

    let system_instruction = crate::gemini::prompt::get_briefing_system_instruction(&greeting_name);
    let final_prompt = format!(
        "It is {}.\n\nRAW DATA CONTEXT:\n{}\n\nTASK:\nGenerate a comprehensive briefing. You MUST synthesize and mention the important emails and calendar events alongside your notes. Do not ignore the financial or deployment alerts if they are present.", 
        current_time_str, 
        raw_data_context
    );

    let briefing_text = gemini_client
        .send_chat(
            vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart::text(final_prompt)],
            }],
            Some(&system_instruction),
            None,
            Some(GenerationConfig {
                response_mime_type: None,
                response_schema: None,
            }),
        )
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|p| p.text.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // 4. Async TTS
    let db_for_audio = database.inner().clone();
    let text_for_audio = briefing_text.clone();
    
    tauri::async_runtime::spawn(async move {
        if let Ok(audio_data) = crate::integrations::gemini_tts::generate_audio(&db_for_audio, &text_for_audio).await {
            let connection = db_for_audio.connection.lock();
            let _ = connection.execute(
                "UPDATE briefing_summaries SET audio_data = ?1 WHERE id = (SELECT MAX(id) FROM briefing_summaries)",
                params![audio_data],
            );
            use tauri::Emitter;
            let _ = app_handle.emit("briefing-audio-ready", ());
        }
    });

    // 5. Save to DB
    {
        let connection = database.connection.lock();
        queries::save_briefing_summary(&connection, &briefing_text, "power-up", None)
            .map_err(|e| e.to_string())?;
    }

    Ok(DashboardBriefing {
        content: briefing_text,
        created_at: Local::now().to_rfc3339(),
        is_stale: false,
        audio_data: None,
    })
}
