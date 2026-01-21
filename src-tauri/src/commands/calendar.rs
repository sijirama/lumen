use crate::database::Database;
use crate::integrations::google_calendar::{self, GoogleCalendarEvent};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Utc};
use tauri::Manager;

#[tauri::command]
pub async fn get_calendar_events_for_range(
    app: tauri::AppHandle,
    start_iso: String,
    end_iso: String,
) -> Result<Vec<GoogleCalendarEvent>, String> {
    let database = app.state::<Database>();

    // Attempt to fetch from Google
    // If it fails (e.g. not connected), we return an empty list or error
    match google_calendar::fetch_google_calendar_events(&database, &start_iso, &end_iso).await {
        Ok(events) => Ok(events),
        Err(e) => {
            // Fallback: check if we have them cached in DB for this range?
            // For now, if Google fails/is-unconfigured, we just return empty list to keep frontend happy
            println!("Calendar fetch error: {}", e);
            Ok(vec![])
        }
    }
}
