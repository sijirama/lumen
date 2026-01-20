use crate::crypto::decrypt_token;
use crate::database::{queries, Database};
use crate::gemini::{
    client::{GeminiContent, GeminiPart},
    GeminiClient,
};
use crate::integrations::google_gmail;
use std::time::Duration;
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

pub async fn start_proactive_agent(app_handle: tauri::AppHandle, database: Database) {
    println!("🚀 Proactive Agent: Starting background loop...");

    loop {
        // Run every 5 minutes
        sleep(Duration::from_secs(300)).await;

        if let Err(e) = check_for_updates(&app_handle, &database).await {
            eprintln!("❌ Proactive Agent Error: {}", e);
        }
    }
}

async fn check_for_updates(
    app_handle: &tauri::AppHandle,
    database: &Database,
) -> anyhow::Result<()> {
    // 1. Check Gmail
    check_gmail(app_handle, database).await?;
    Ok(())
}

async fn check_gmail(app_handle: &tauri::AppHandle, database: &Database) -> anyhow::Result<()> {
    // Check if Google integration is enabled
    let has_google = {
        let connection = database.connection.lock();
        queries::get_integration(&connection, "google")?
            .map(|i| i.enabled)
            .unwrap_or(false)
    };

    if !has_google {
        return Ok(());
    }

    // Get unread emails
    let emails = google_gmail::fetch_recent_emails(database, 5).await?;

    for email in emails {
        let already_notified = {
            let connection = database.connection.lock();
            queries::has_notified(&connection, &email.id, "gmail")?
        };

        if already_notified {
            continue;
        }

        // Triage with Gemini
        if should_notify_for_email(database, &email).await? {
            let title = email
                .subject
                .clone()
                .unwrap_or_else(|| "New Email".to_string());

            // 1. Send System Notification
            app_handle
                .notification()
                .builder()
                .title("Lumen: Meaningful Update")
                .body(&format!("{}", title))
                .show()?;

            // 2. Generate and Save Proactive Chat Message
            let assistant_text = generate_proactive_message(database, &email).await?;
            {
                use crate::database::queries::ChatMessage;
                let connection = database.connection.lock();
                let msg = ChatMessage {
                    id: None,
                    role: "assistant".to_string(),
                    content: assistant_text.clone(),
                    image_data: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    session_id: None,
                };
                let msg_id = queries::save_chat_message(&connection, &msg)?;

                // Emit event to update UI live
                use tauri::Emitter;
                let now_str = chrono::Utc::now().to_rfc3339();
                let _ = app_handle.emit(
                    "assistant-message",
                    crate::commands::chat::ChatMessageResponse {
                        id: Some(msg_id),
                        role: "assistant".to_string(),
                        content: assistant_text,
                        image_data: None,
                        created_at: now_str,
                    },
                );
            }

            // Record in DB to avoid double notification
            {
                let connection = database.connection.lock();
                queries::record_notification(&connection, &email.id, "gmail", &title)?;
            }

            println!("🔔 Proactive Agent: Notified for email '{}'", title);
        } else {
            // Record so we don't ask Gemini again for the same skip
            let connection = database.connection.lock();
            queries::record_notification(&connection, &email.id, "gmail", "SKIPPED")?;
        }
    }

    Ok(())
}

async fn generate_proactive_message(
    database: &Database,
    email: &google_gmail::GmailMessage,
) -> anyhow::Result<String> {
    let api_key = {
        let connection = database.connection.lock();
        let encrypted_key = queries::get_api_token(&connection, "gemini")?
            .ok_or_else(|| anyhow::anyhow!("Gemini key missing"))?;
        decrypt_token(&encrypted_key)?
    };

    let client = GeminiClient::new(api_key);
    let prompt = format!(
        "As Lumen, a soft and kind desktop sidekick, write a very brief (1-2 sentences) chat message to the user about this email. 
        Be warm and professional. Use an emoji.
        
        EMAIL:
        From: {:?}
        Subject: {:?}
        Snippet: {:?}",
        email.from, email.subject, email.snippet
    );

    let response = client
        .send_chat(
            vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart::text(prompt)],
            }],
            None,
            None,
        )
        .await?;

    Ok(response
        .first()
        .and_then(|p| p.text.clone())
        .unwrap_or_else(|| "I noticed a new email you might want to check! 📧".to_string()))
}

async fn should_notify_for_email(
    database: &Database,
    email: &google_gmail::GmailMessage,
) -> anyhow::Result<bool> {
    let api_key = {
        let connection = database.connection.lock();
        let encrypted_key = queries::get_api_token(&connection, "gemini")?
            .ok_or_else(|| anyhow::anyhow!("Gemini key missing"))?;
        decrypt_token(&encrypted_key)?
    };

    let client = GeminiClient::new(api_key);
    let prompt = format!(
        "As Lumen, a kind and observant sidekick, triage this new email to see if it warrants a gentle desktop ping.
        
        EMAIL DETAILS:
        From: {:?}
        Subject: {:?}
        Snippet: {:?}
        
        Ping the user ONLY if:
        1. It is a personal message from a human.
        2. It is an urgent request or important update.
        3. It relates to the user's focus (Lumen, coding, personal projects).
        
        IGNORE: Newsletters, marketing, automated alerts (unless critical), or noise.
        
        Response: ONLY say 'YES' or 'NO'.",
        email.from, email.subject, email.snippet
    );

    let response = client
        .send_chat(
            vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart::text(prompt)],
            }],
            None,
            None,
        )
        .await?;

    let response_text = response
        .first()
        .and_then(|p| p.text.clone())
        .unwrap_or_default()
        .to_uppercase();

    Ok(response_text.contains("YES"))
}
