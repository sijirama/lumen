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
