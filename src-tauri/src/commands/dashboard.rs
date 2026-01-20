//INFO: Dashboard commands for Lumen
//NOTE: Handles daily briefing summaries with hashing and AI evolution

use crate::database::{queries, Database};
use crate::gemini::client::{GeminiClient, GeminiContent, GeminiPart};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Local};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardBriefing {
    pub content: String,
    pub created_at: String,
    pub is_stale: bool,
    pub audio_data: Option<String>, // Base64 encoded audio
}

//INFO: Gets the latest briefing from the database and checks if it's stale
#[tauri::command]
pub async fn get_dashboard_briefing(
    database: State<'_, Database>,
) -> Result<Option<DashboardBriefing>, String> {
    // Get latest summary in a scoped block to release the lock immediately
    let latest = {
        let connection = database.connection.lock();
        queries::get_latest_briefing_summary(&connection).map_err(|e| e.to_string())?
    };

    // Calculate current hash (this is async)
    let current_hash = calculate_briefing_hash(&database).await?;

    if let Some(summary) = latest {
        let b64_audio = summary
            .audio_data
            .map(|data| general_purpose::STANDARD.encode(data));

        Ok(Some(DashboardBriefing {
            content: summary.content,
            created_at: summary.created_at,
            is_stale: summary.data_hash != current_hash,
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
) -> Result<DashboardBriefing, String> {
    let current_hash = calculate_briefing_hash(&database).await?;

    // 1. Get Context (Yesterday's final, Today's history)
    let context = {
        let connection = database.connection.lock();
        let mut parts = Vec::new();

        if let Ok(Some(yesterday)) = queries::get_yesterdays_final_briefing(&connection) {
            parts.push(format!("Yesterday's final outcome: {}", yesterday.content));
        }

        let todays = queries::get_todays_briefings(&connection).unwrap_or_default();
        if !todays.is_empty() {
            parts.push("Today's narrative evolution so far:".to_string());
            for (i, b) in todays.iter().enumerate() {
                parts.push(format!("{}. {}", i + 1, b.content));
            }
        }
        parts.join("\n\n")
    };

    // 2. Get Raw Data (Current daily note + calendar + weather)
    let (location_name, greeting_name) = {
        let connection = database.connection.lock();
        let profile = queries::get_user_profile(&connection).ok().flatten();
        (
            profile
                .as_ref()
                .and_then(|p| p.location.clone())
                .unwrap_or_else(|| "Lagos".to_string()),
            profile
                .as_ref()
                .map(|p| p.display_name.clone())
                .unwrap_or_else(|| "User".to_string()),
        )
    };

    let weather = crate::gemini::tools::fetch_weather(&location_name).await;

    let raw_data = {
        let connection = database.connection.lock();
        let mut data = Vec::new();

        // Weather
        if !weather.get("error").is_some() {
            data.push(format!(
                "Current Weather in {}:\n{}",
                location_name, weather
            ));
        }

        // Obsidian - Hunt for last available note if today's is missing
        if let Ok(Some(integration)) = queries::get_integration(&connection, "obsidian") {
            if integration.enabled {
                if let Some(config) = integration.config {
                    if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                        if let Some(vault_path) =
                            config_json.get("vault_path").and_then(|v| v.as_str())
                        {
                            let daily_notes_folder = config_json
                                .get("daily_notes_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let date_format = config_json
                                .get("daily_notes_format")
                                .and_then(|v| v.as_str())
                                .unwrap_or("YYYY-MM-DD");
                            let chrono_format = date_format
                                .replace("YYYY", "%Y")
                                .replace("MM", "%m")
                                .replace("DD", "%d");

                            data.push(format!("Obsidian Vault Root: {}", vault_path));

                            // Try today, then go back up to 7 days
                            let mut found_note = false;
                            for i in 0..7 {
                                let target_date = Local::now() - Duration::days(i);
                                let daily_note_name =
                                    format!("{}.md", target_date.format(&chrono_format));
                                let daily_note_path = Path::new(vault_path)
                                    .join(daily_notes_folder)
                                    .join(&daily_note_name);

                                if let Ok(content) = fs::read_to_string(&daily_note_path) {
                                    // Strip unwanted sections
                                    let mut cleaned_lines = Vec::new();
                                    let mut skipping = false;
                                    for line in content.lines() {
                                        let l = line.to_lowercase();
                                        if l.contains("notes created today")
                                            || l.contains("notes last touched today")
                                        {
                                            skipping = true;
                                        } else if skipping && line.starts_with("#") {
                                            // Stop skipping if we hit a new header
                                            skipping = false;
                                            cleaned_lines.push(line);
                                        } else if !skipping {
                                            cleaned_lines.push(line);
                                        }
                                    }
                                    let cleaned_content = cleaned_lines.join("\n");

                                    data.push(format!(
                                        "Daily Note (Path: {}):\n{}",
                                        daily_note_path.display(),
                                        cleaned_content
                                    ));
                                    found_note = true;
                                    break;
                                }
                            }
                            if !found_note {
                                data.push("No recent Obsidian daily notes found.".to_string());
                            }
                        }
                    }
                }
            }
        }

        data.join("\n\n")
    };

    // 2.5 Fetch Live Google Data (Outside lock to be thread-safe)
    let mut live_data = Vec::new();
    if let Ok(has_google) = {
        let connection = database.connection.lock();
        queries::has_api_token(&connection, "google")
    } {
        if has_google {
            // Fetch Calendar
            let start_of_day = Local::now().format("%Y-%m-%dT00:00:00Z").to_string();
            let end_of_day = Local::now().format("%Y-%m-%dT23:59:59Z").to_string();

            if let Ok(events) = crate::integrations::google_calendar::fetch_google_calendar_events(
                &database,
                &start_of_day,
                &end_of_day,
            )
            .await
            {
                if !events.is_empty() {
                    let mut e_str = String::from("Today's Real Calendar Events (from Google):\n");
                    for e in events {
                        let start = e
                            .start
                            .date_time
                            .as_deref()
                            .or(e.start.date.as_deref())
                            .unwrap_or("unknown");
                        e_str.push_str(&format!("- {} (starts at {})\n", e.summary, start));
                    }
                    live_data.push(e_str);
                }
            }

            // Fetch Emails (Use precise Unix timestamp for "today" to avoid timezone issues)
            let start_of_day = Local::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .timestamp();

            let today_query = format!("after:{}", start_of_day);
            if let Ok(emails) = crate::integrations::google_gmail::fetch_recent_emails_with_query(
                &database,
                10,
                Some(&today_query),
            )
            .await
            {
                if !emails.is_empty() {
                    let mut m_str = String::from("Emails from today:\n");
                    for m in emails {
                        m_str.push_str(&format!(
                            "- From: {} | Subject: {} | Snippet: {}\n",
                            m.from.as_deref().unwrap_or("Unknown"),
                            m.subject.as_deref().unwrap_or("No Subject"),
                            m.snippet
                        ));
                    }
                    live_data.push(m_str);
                }
            }

            // Fetch Tasks
            if let Ok(tasks) = crate::integrations::google_tasks::list_tasks(&database, 10).await {
                if !tasks.is_empty() {
                    let mut t_str = String::from("Pending Tasks (from Google Tasks):\n");
                    for t in tasks {
                        t_str.push_str(&format!("- {} (status: {})\n", t.title, t.status));
                    }
                    live_data.push(t_str);
                }
            }
        }
    }

    let final_raw_data = if live_data.is_empty() {
        raw_data
    } else {
        format!("{}\n\n{}", raw_data, live_data.join("\n\n"))
    };

    // 3. Call Gemini
    let api_key = {
        let connection = database.connection.lock();
        queries::get_api_token(&connection, "gemini")
            .map_err(|e| e.to_string())?
            .ok_or("Gemini API key not configured")?
    };

    let decrypted_key = crate::crypto::decrypt_token(&api_key).map_err(|e| e.to_string())?;
    let client = GeminiClient::new(decrypted_key);

    let system_instruction = format!("You are Lumen, a soft, kind, and observant companion for {}.
    
    YOUR MISSION: 
    Provide a gentle, supportive, and tactical overview of the day.

    CRITICAL INSTRUCTIONS:
    - ANALYZE: Softly weave together connections between Obsidian notes, unread Emails, and Calendar events.
    - PRIORITIZE: Help the user find their focus today by identifying the most meaningful 'Lead Domino' in a calm, encouraging way.
    - TIME-AWARENESS: It is currently {}. Be warm and gentle in your greeting. In the morning, provide quiet encouragement. In the evening, help the user reflect and transition to rest.
    - NO COMPLAINING: Never mention missing data. Focus on the beauty of what is present.
    - FORMAT: 
      - NO TITLES OR HEADINGS: Do not use any headings, titles, or labels for sections (no \"###\", \"##\", or bolded titles). This is a single, flowing briefing.
      - STRUCTURE: Use two empty lines to separate distinct topics or contexts for a breathable, minimalist layout.
      - INSIGHTS: Use normal text for everything. DO NOT use blockquotes ('>') or any special highlighting.
      - LINKS: Use [Name](<lumen://open?path=/absolute/path>) for all specific notes or files mentioned. IMPORTANT: You MUST wrap the URL in angle brackets `< >` because paths often contain spaces (e.g., [Daily Note](<lumen://open?path=/User/Notes/Daily Notes/Note.md>)).
      - TONE: Minimal, clean, and deeply supportive. NO ITALICS. NO BOLDING.", 
    greeting_name,
    Local::now().format("%I:%M %p")
    );

    let prompt = format!(
        "RAW DATA CONTEXT:\n{}\n\nHISTORY CONTEXT:\n{}\n\nGenerate the briefing. Remember: be proactive, never ask questions or list missing data.",
        final_raw_data, context
    );

    let response_text = client
        .send_chat(
            vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart::text(prompt)],
            }],
            Some(&system_instruction),
            None,
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

    // 4. Generate Audio (Gemini TTS)
    let audio_data = crate::integrations::gemini_tts::generate_audio(&database, &response_text)
        .await
        .ok(); // Fallback if TTS fails

    // 5. Save to DB
    {
        let connection = database.connection.lock();
        queries::save_briefing_summary(
            &connection,
            &response_text,
            &current_hash,
            audio_data.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    }

    let b64_audio = audio_data.map(|data| general_purpose::STANDARD.encode(data));

    Ok(DashboardBriefing {
        content: response_text,
        created_at: Local::now().to_rfc3339(),
        is_stale: false,
        audio_data: b64_audio,
    })
}

//INFO: Calculates a hash of the current data sources to detect changes
async fn calculate_briefing_hash(database: &State<'_, Database>) -> Result<String, String> {
    let mut hash_input = String::new();
    let today = Local::now();

    {
        let connection = database.connection.lock();

        // 1. Obsidian Meta
        if let Ok(Some(integration)) = queries::get_integration(&connection, "obsidian") {
            if integration.enabled {
                if let Some(config) = integration.config {
                    if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                        if let Some(vault_path) =
                            config_json.get("vault_path").and_then(|v| v.as_str())
                        {
                            let daily_notes_folder = config_json
                                .get("daily_notes_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let date_format = config_json
                                .get("daily_notes_format")
                                .and_then(|v| v.as_str())
                                .unwrap_or("YYYY-MM-DD");
                            let chrono_format = date_format
                                .replace("YYYY", "%Y")
                                .replace("MM", "%m")
                                .replace("DD", "%d");
                            let daily_note_name = format!("{}.md", today.format(&chrono_format));
                            let daily_note_path = Path::new(vault_path)
                                .join(daily_notes_folder)
                                .join(&daily_note_name);

                            if let Ok(metadata) = fs::metadata(daily_note_path) {
                                if let Ok(modified) = metadata.modified() {
                                    hash_input.push_str(&format!(
                                        "obsidian:{:?}:{}",
                                        modified,
                                        metadata.len()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Google Meta (Calendar & Gmail)
        if let Ok(has_google) = queries::has_api_token(&connection, "google") {
            if has_google {
                if let Ok(Some(integration)) = queries::get_integration(&connection, "google") {
                    let last_sync = integration.last_sync.clone().unwrap_or_default();
                    hash_input.push_str(&format!(
                        "google:{}:{}",
                        last_sync,
                        today.format("%Y-%m-%d")
                    ));
                } else {
                    hash_input.push_str(&format!("google:connected:{}", today.format("%Y-%m-%d")));
                }
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(hash_input);
    Ok(format!("{:x}", hasher.finalize()))
}
