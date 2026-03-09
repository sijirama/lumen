//INFO: Memory extractor - generates observations from chat, clipboard, and briefings
//NOTE: Uses latent mod-50 triggers to batch extraction efficiently

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::memory::core::{MemoryItem, MemoryType};

//INFO: Creates a MemoryItem from raw extracted data
pub fn create_memory(
    memory_type: MemoryType,
    content: String,
    importance: f64,
) -> MemoryItem {
    let now = Utc::now();
    MemoryItem {
        id: Uuid::new_v4().to_string(),
        memory_type,
        content,
        importance,
        created_at: now,
        last_accessed: now,
        access_count: 0,
        embedding: None,
        score: 0.0,
    }
}

//INFO: Build the extraction prompt for chat batches
pub fn build_chat_extraction_prompt(messages: &[String]) -> String {
    let chat_block = messages.join("\n");
    format!(
        r#"You are a memory extraction agent for a personal AI assistant called Lumen.

Below is a batch of recent chat messages between the user and Lumen.
Your job is to extract HIGHLY DETAILED observations, preferences, and entities from this conversation.

RULES:
- Extract as many important memories as you can find.
- Each memory must be VERY DETAILED so semantic search can find it later.
- Score each memory's importance from 1-10.
- Types: "observation" (facts, events, activities), "preference" (user likes/dislikes), "entity" (named things: projects, people, accounts, tools).
- Return ONLY valid JSON array.

FORMAT:
[
  {{"type": "observation", "content": "detailed description...", "importance": 7}},
  {{"type": "entity", "content": "detailed description...", "importance": 8}}
]

CHAT MESSAGES:
{}

Extract all important memories now:"#,
        chat_block
    )
}

//INFO: Build the extraction prompt for clipboard batches
pub fn build_clipboard_extraction_prompt(items: &[String]) -> String {
    let clipboard_block = items.join("\n---\n");
    format!(
        r#"You are a memory extraction agent for a personal AI assistant called Lumen.

Below is a batch of recent clipboard items the user has copied.
Your job is to extract HIGHLY DETAILED observations or preferences about the user's ongoing work.

RULES:
- Extract as many important memories as you can.
- Each memory must be VERY DETAILED.
- Score importance 1-10.
- Types: "observation", "preference", "entity".
- Return ONLY valid JSON array.

FORMAT:
[
  {{"type": "observation", "content": "detailed description...", "importance": 6}}
]

CLIPBOARD ITEMS:
{}

Extract all important memories now:"#,
        clipboard_block
    )
}

//INFO: Parsed memory from LLM extraction response
#[derive(Debug, serde::Deserialize)]
pub struct ExtractedMemory {
    #[serde(rename = "type")]
    pub memory_type: String,
    pub content: String,
    pub importance: f64,
}

//INFO: Parse the LLM's JSON response into MemoryItems
pub fn parse_extracted_memories(json_response: &str) -> Result<Vec<MemoryItem>> {
    let extracted: Vec<ExtractedMemory> = serde_json::from_str(json_response.trim())?;

    let memories = extracted
        .into_iter()
        .filter_map(|e| {
            let mem_type = MemoryType::from_str(&e.memory_type)?;
            Some(create_memory(mem_type, e.content, e.importance))
        })
        .collect();

    Ok(memories)
}
