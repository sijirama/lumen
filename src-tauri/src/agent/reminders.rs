use crate::database::Database;
use crate::database::queries::get_active_reminders;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_notification::NotificationExt;
use std::sync::Arc;
use tokio::sync::Notify;
use chrono::{DateTime, Utc};
use std::time::Duration;

//INFO: Reminder manager to handle wake-ups and state
pub struct ReminderManager {
    pub notify: Arc<Notify>,
}

impl ReminderManager {
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn trigger_update(&self) {
        self.notify.notify_waiters();
    }
}

//INFO: Background daemon that manages reminder timing and notifications
pub async fn start_reminder_daemon(app: AppHandle, db: Database) {
    let reminder_manager = app.state::<ReminderManager>();
    let notify = reminder_manager.notify.clone();

    crate::applog!("DEBUG: 🔔 Reminder daemon started.");

    loop {
        // 1. Get the next upcoming reminder
        let next_reminder = {
            let connection = db.connection.lock();
            match get_active_reminders(&connection) {
                Ok(reminders) => {
                    // Filter for those with due_at and find the earliest
                    reminders.into_iter()
                        .filter(|r| r.due_at.is_some())
                        .min_by_key(|r| r.due_at.clone().unwrap())
                },
                Err(e) => {
                    eprintln!("ERROR: Failed to fetch reminders: {}", e);
                    None
                }
            }
        };

        // 2. Decide how long to sleep
        match next_reminder {
            Some(reminder) => {
                let due_at = DateTime::parse_from_rfc3339(reminder.due_at.as_ref().unwrap())
                    .unwrap()
                    .with_timezone(&Utc);
                
                let now = Utc::now();
                
                if due_at <= now {
                    // Trigger notification immediately
                    crate::applog!("DEBUG: 🔔 Triggering reminder: {}", reminder.content);
                    
                    let _ = app.notification()
                        .builder()
                        .title("Lumen Reminder")
                        .body(&reminder.content)
                        .show();
                    
                    // Mark as completed locally or just wait for next loop
                    // For now, let's mark it so we don't spam
                    let connection = db.connection.lock();
                    let _ = crate::database::queries::toggle_reminder_completion(&connection, reminder.id, true);
                    let _ = app.emit("reminders-updated", ());
                } else {
                    let sleep_duration = (due_at - now).to_std().unwrap_or(Duration::from_secs(1));
                    crate::applog!("DEBUG: 🔔 Next reminder in {:?}: {}", sleep_duration, reminder.content);
                    
                    // Sleep until either the reminder is due OR we get a notification boost
                    tokio::select! {
                        _ = tokio::time::sleep(sleep_duration) => {
                            // Time reached, loop around to trigger
                        }
                        _ = notify.notified() => {
                            // Something changed in the DB, restart the loop to re-scan
                            crate::applog!("DEBUG: 🔔 Reminder list updated, re-scanning...");
                        }
                    }
                }
            },
            None => {
                // No reminders, wait indefinitely until notified
                crate::applog!("DEBUG: 🔔 No upcoming reminders. Sleeping...");
                notify.notified().await;
                crate::applog!("DEBUG: 🔔 Waking up to check new reminders...");
            }
        }
    }
}
