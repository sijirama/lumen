//INFO: Lumen Scheduler — proactive always-on background tasks
//NOTE: Runs morning briefing push and calendar-aware reminder sync

use crate::database::Database;
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Utc};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

//INFO: Start the main scheduler loop — runs daily tasks in the background
pub async fn start_scheduler(app: AppHandle, db: Database) {
    println!("DEBUG: ⏰ Scheduler started.");

    // Sync calendar reminders once on startup
    sync_calendar_reminders(&app, &db).await;

    loop {
        let now = Local::now();
        let hour = now.hour();
        let minute = now.minute();

        // Morning briefing push at 8:00am
        if hour == 8 && minute == 0 {
            push_morning_briefing(&app, &db).await;
        }

        // Re-sync calendar reminders every hour on the hour
        if minute == 0 {
            sync_calendar_reminders(&app, &db).await;
        }

        // Sleep until the start of the next minute
        let seconds_until_next_minute = 60 - now.second();
        tokio::time::sleep(std::time::Duration::from_secs(seconds_until_next_minute as u64)).await;
    }
}

//INFO: Push a morning briefing notification — fires at 8am
async fn push_morning_briefing(app: &AppHandle, db: &Database) {
    println!("DEBUG: ⏰ Morning briefing push triggered.");

    // Get user name
    let user_name = {
        let conn = db.connection.lock();
        crate::database::queries::get_user_profile(&conn)
            .ok()
            .flatten()
            .map(|p| p.display_name)
            .unwrap_or_else(|| "there".to_string())
    };

    // Show system notification to open the dashboard
    let _ = app
        .notification()
        .builder()
        .title("Good morning ✨")
        .body(&format!("Hey {}, your daily briefing is ready.", user_name))
        .show();

    // Emit event to frontend to refresh briefing if window is open
    let _ = app.emit("morning-briefing-ready", ());

    println!("DEBUG: ⏰ Morning briefing notification sent.");
}

//INFO: Sync today's calendar events into the reminders system (15 min pre-event alerts)
pub async fn sync_calendar_reminders(app: &AppHandle, db: &Database) {
    println!("DEBUG: ⏰ Syncing calendar reminders...");

    let events = {
        let conn = db.connection.lock();
        crate::database::queries::get_todays_calendar_events(&conn).unwrap_or_default()
    };

    let now = Utc::now();
    let mut created = 0;

    for event in &events {
        // Skip all-day events
        if event.all_day {
            continue;
        }

        // Parse event start time
        let start_time = match DateTime::parse_from_rfc3339(&event.start_time) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => continue,
        };

        // Only care about future events
        if start_time <= now {
            continue;
        }

        // Reminder fires 15 minutes before
        let reminder_time = start_time - Duration::minutes(15);

        // Skip if reminder time is already past
        if reminder_time <= now {
            continue;
        }

        // Check if reminder already exists for this event
        let already_exists = {
            let conn = db.connection.lock();
            crate::database::queries::calendar_reminder_exists(&conn, &event.id)
        };

        if already_exists {
            continue;
        }

        // Create a reminder
        let content = format!(
            "📅 {} starts in 15 minutes{} [cal:{}]",
            event.title,
            event.location.as_deref().map(|l| format!(" at {}", l)).unwrap_or_default(),
            event.id
        );
        let due_at = reminder_time.to_rfc3339();

        {
            let conn = db.connection.lock();
            if let Ok(()) = crate::database::queries::create_reminder(&conn, &content, Some(&due_at)) {
                created += 1;
            }
        }
    }

    if created > 0 {
        println!("DEBUG: ⏰ Created {} calendar reminder(s).", created);
        // Wake up the reminder daemon to pick up new reminders
        if let Some(manager) = app.try_state::<crate::agent::reminders::ReminderManager>() {
            manager.trigger_update();
        }
    } else {
        println!("DEBUG: ⏰ No new calendar reminders needed.");
    }
}
