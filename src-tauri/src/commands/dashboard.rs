//INFO: Dashboard commands for Lumen
//NOTE: Handles daily briefing summaries with hashing and AI evolution

use crate::database::{queries, Database};
use crate::gemini::client::{GeminiClient, GeminiContent, GeminiPart};
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
        Ok(Some(DashboardBriefing {
            content: summary.content,
            created_at: summary.created_at,
            is_stale: summary.data_hash != current_hash,
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

                            // Try today, then go back up to 7 days
                            let mut found_note = false;
                            for i in 0..7 {
                                let target_date = Local::now() - Duration::days(i);
                                let daily_note_name =
                                    format!("{}.md", target_date.format(&chrono_format));
                                let daily_note_path = Path::new(vault_path)
                                    .join(daily_notes_folder)
                                    .join(&daily_note_name);

                                if let Ok(content) = fs::read_to_string(daily_note_path) {
                                    data.push(format!(
                                        "Available Daily Note ({}{}):\n{}",
                                        if i == 0 { "TODAY" } else { "PAST" },
                                        if i > 0 {
                                            format!(" - {} days ago", i)
                                        } else {
                                            "".to_string()
                                        },
                                        content
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

        // Calendar (Mocked/Cached)
        let today = Local::now();
        let start_of_day = today.format("%Y-%m-%dT00:00:00").to_string();
        let end_of_day = today.format("%Y-%m-%dT23:59:59").to_string();
        if let Ok(events) = queries::get_calendar_events(&connection, &start_of_day, &end_of_day) {
            if !events.is_empty() {
                let mut e_str = String::from("Today's Calendar Events:\n");
                for e in events {
                    e_str.push_str(&format!(
                        "- {} ({} to {})\n",
                        e.title, e.start_time, e.end_time
                    ));
                }
                data.push(e_str);
            }
        }

        data.join("\n\n")
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

    let system_instruction = format!("You are Lumen, a witty and proactive desktop agent. Your task is to generate a 'Daily Briefing' for the user, {}.

CRITICAL INSTRUCTIONS:
- NEVER EVER ask the user for missing information. If data is lacking, DO NOT MENTION IT. 
- You MUST provide a briefing based ONLY on the available data. 
- If no daily notes or calendar events are found, FOCUS HEAVILY on the weather and provide a witty, cheerful greeting and sign-off.
- The briefing should be a narrative, not a status report on missing data.
- NEVER list the categories you need (e.g., 'Yesterday's Summary', 'Calendar Events').
- Be concise (2-3 short paragraphs max).
- Use Markdown for formatting.

Tone: Premium, witty, and deeply helpful.", greeting_name);

    let prompt = format!(
        "RAW DATA CONTEXT:\n{}\n\nHISTORY CONTEXT:\n{}\n\nGenerate the briefing. Remember: be proactive, never ask questions or list missing data.",
        raw_data, context
    );

    let response_text = client
        .send_chat(
            vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart {
                    text: Some(prompt),
                    function_call: None,
                    function_response: None,
                }],
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

    // 4. Save to DB
    {
        let connection = database.connection.lock();
        queries::save_briefing_summary(&connection, &response_text, &current_hash)
            .map_err(|e| e.to_string())?;
    }

    Ok(DashboardBriefing {
        content: response_text,
        created_at: Local::now().to_rfc3339(),
        is_stale: false,
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

        // 2. Calendar Meta
        if let Ok(Some(integration)) = queries::get_integration(&connection, "google_calendar") {
            if integration.enabled {
                let last_sync = integration.last_sync.clone().unwrap_or_default();
                let start_of_day = today.format("%Y-%m-%dT00:00:00").to_string();
                let end_of_day = today.format("%Y-%m-%dT23:59:59").to_string();
                if let Ok(events) =
                    queries::get_calendar_events(&connection, &start_of_day, &end_of_day)
                {
                    hash_input.push_str(&format!(
                        "calendar:{}:{}:{}",
                        events.len(),
                        last_sync,
                        today.format("%Y-%m-%d")
                    ));
                }
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(hash_input);
    Ok(format!("{:x}", hasher.finalize()))
}
