//INFO: Memory inspector commands — let the user SEE and prune what Lumen
//      remembers about them. Privacy/trust surface for the memory subsystem.

use crate::crypto::decrypt_token;
use crate::database::queries::get_api_token;
use crate::database::Database;
use crate::gemini::client::GeminiClient;
use crate::memory::core::{self, MemoryItem, MemoryType};
use crate::memory::extractor::create_memory;
use tauri::State;

//INFO: Pull + decrypt the Gemini key (needed to embed new/edited memories).
fn gemini_key(db: &Database) -> Result<String, String> {
    let conn = db.connection.lock();
    get_api_token(&conn, "gemini")
        .ok()
        .flatten()
        .and_then(|enc| decrypt_token(&enc).ok())
        .ok_or_else(|| "Gemini API key not configured".to_string())
}

//INFO: Lists what Lumen remembers, newest first (default 200).
#[tauri::command]
pub fn get_memories(
    database: State<Database>,
    limit: Option<i64>,
) -> Result<Vec<MemoryItem>, String> {
    let connection = database.connection.lock();
    core::list_all_memories(&connection, limit.unwrap_or(200) as usize)
        .map_err(|e| format!("Failed to list memories: {}", e))
}

//INFO: Deletes a single memory (and its embedding).
#[tauri::command]
pub fn delete_memory(database: State<Database>, id: String) -> Result<(), String> {
    let connection = database.connection.lock();
    core::delete_memory(&connection, &id).map_err(|e| format!("Failed to delete memory: {}", e))
}

//INFO: Forgets everything — wipes all memories and embeddings.
#[tauri::command]
pub fn clear_all_memories(database: State<Database>) -> Result<(), String> {
    let connection = database.connection.lock();
    core::clear_all_memories(&connection).map_err(|e| format!("Failed to clear memories: {}", e))
}

//INFO: Manually add a memory the user typed in (the UI counterpart to the
//      remember_this tool). Embeds it so it's retrievable. Defaults to a
//      high-importance preference since the user chose to record it deliberately.
#[tauri::command]
pub async fn add_memory(
    database: State<'_, Database>,
    content: String,
    memory_type: Option<String>,
) -> Result<(), String> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("Memory content is empty".to_string());
    }
    let db = database.inner().clone();
    let api_key = gemini_key(&db)?;

    let mem_type = MemoryType::from_str(memory_type.as_deref().unwrap_or("preference"))
        .unwrap_or(MemoryType::Preference);
    let mut memory = create_memory(mem_type, content.clone(), 8.0);

    if let Ok(emb) = GeminiClient::new(api_key).generate_embedding(&content).await {
        memory.embedding = Some(emb);
    }

    let conn = db.connection.lock();
    core::store_memory(&conn, &memory).map_err(|e| format!("Failed to save memory: {}", e))
}

//INFO: Edit a memory's text and re-embed it so retrieval stays accurate.
#[tauri::command]
pub async fn update_memory(
    database: State<'_, Database>,
    id: String,
    content: String,
) -> Result<(), String> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("Memory content is empty".to_string());
    }
    let db = database.inner().clone();
    let api_key = gemini_key(&db)?;

    let embedding = GeminiClient::new(api_key).generate_embedding(&content).await.ok();

    let conn = db.connection.lock();
    core::update_memory_content(&conn, &id, &content, embedding.as_deref())
        .map_err(|e| format!("Failed to update memory: {}", e))
}
