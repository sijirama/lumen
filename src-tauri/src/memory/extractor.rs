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
pub fn build_chat_extraction_prompt(messages: &[String], user_name: &str) -> String {
    let chat_block = messages.join("\n");
    format!(
        r#"You are Lumen's internal memory extraction engine. Lumen is the AI sidekick, and {} is the user.

Below is a batch of recent chat messages. 
Your job is to extract STARK, HIGH-DENSITY, and EXTREMELY DETAILED memories.

RULES:
- NO REDUNDANCY: Do not extract information that is already common knowledge or redundant within this batch.
- DENSITY: Combine related facts into a single, comprehensive, and information-dense observation rather than multiple small ones.
- NAMES: Use " {}" for the user and "Lumen" for the assistant in the content. NEVER say "the user" or "the AI".
- STARK & DETAILED: Each memory must be a standalone, high-fidelity fact that can be used for deep reasoning later.
- Types: 
  - "observation": Sharp facts, events, technical skills, or activities.
  - "preference": Specific likes, dislikes, or personal goals of {}.
  - "entity": Critical named things (projects, unique tools, specific people, deep-tech concepts).

Return ONLY a valid JSON array of these high-value memories.

FORMAT EXAMPLE:
[
  {{"type": "observation", "content": "{} is implementing a custom MoE architecture inspired by DeepSeek-V3, focusing on low-level CUDA optimizations.", "importance": 9}},
  {{"type": "preference", "content": "{} prioritizes efficiency and system-level performance over high-level abstraction in ML research.", "importance": 8}}
]

CHAT MESSAGES:
{}

Extract only unique, dense, and high-importance memories now:"#,
        user_name, user_name, user_name, user_name, user_name, chat_block
    )
}

//INFO: Build the extraction prompt for clipboard batches
pub fn build_clipboard_extraction_prompt(items: &[String], user_name: &str) -> String {
    let clipboard_block = items.join("\n---\n");
    format!(
        r#"You are Lumen's internal memory extraction engine. Lumen is the AI sidekick, and {} is the user.

Below are clips from {}'s clipboard. 
Your job is to extract STARK, HIGH-DENSITY, and EXTREMELY DETAILED observations or preferences about {}'s ongoing work.

RULES:
- NO REDUNDANCY: Do not extract redundant or trivial information.
- DENSITY: Combine snippets into cohesive, information-dense observations.
- NAMES: Use "{}" for the user and "Lumen" for the assistant. NEVER say "the user".
- STARK & DETAILED: Each memory must be a detailed, standalone fact.
- Types: "observation", "preference", "entity".

Return ONLY a valid JSON array.

FORMAT:
[
  {{"type": "observation", "content": "{} is researching specific CUDA kernels for sparse matrix multiplication as part of their ML systems dive.", "importance": 7}}
]

CLIPBOARD ITEMS:
{}

Extract only unique and high-value memories now:"#,
        user_name, user_name, user_name, user_name, user_name, clipboard_block
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
