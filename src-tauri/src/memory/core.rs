//INFO: Core memory structures and retrieval engine for Lumen
//NOTE: Implements the Generative Agents scoring function: Score = Recency + Importance + Relevance

use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zerocopy::AsBytes;

//INFO: Memory types that Lumen can create and store
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    #[serde(rename = "observation")]
    Observation,
    #[serde(rename = "reflection")]
    Reflection,
    #[serde(rename = "entity")]
    Entity,
    #[serde(rename = "preference")]
    Preference,
    #[serde(rename = "daily_summary")]
    DailySummary,
}

impl MemoryType {
    pub fn as_str(&self) -> &str {
        match self {
            MemoryType::Observation => "observation",
            MemoryType::Reflection => "reflection",
            MemoryType::Entity => "entity",
            MemoryType::Preference => "preference",
            MemoryType::DailySummary => "daily_summary",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "observation" => Some(MemoryType::Observation),
            "reflection" => Some(MemoryType::Reflection),
            "entity" => Some(MemoryType::Entity),
            "preference" => Some(MemoryType::Preference),
            "daily_summary" => Some(MemoryType::DailySummary),
            _ => None,
        }
    }
}

//INFO: A single memory item stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub importance: f64,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: i32,
    // Runtime-only fields (not stored directly in DB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub score: f64,
}

//INFO: Store a new memory with its embedding into the database
pub fn store_memory(conn: &Connection, memory: &MemoryItem) -> Result<()> {
    let tx = conn.unchecked_transaction().context("Failed to begin transaction")?;

    tx.execute(
        "INSERT OR REPLACE INTO memories (id, type, content, importance, created_at, last_accessed, access_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            memory.id,
            memory.memory_type.as_str(),
            memory.content,
            memory.importance,
            memory.created_at.to_rfc3339(),
            memory.last_accessed.to_rfc3339(),
            memory.access_count,
        ],
    )
    .context("Failed to insert memory")?;

    // Store embedding if available
    if let Some(ref embedding) = memory.embedding {
        tx.execute(
            "INSERT OR REPLACE INTO memory_embeddings (id, embedding) VALUES (?1, ?2)",
            rusqlite::params![memory.id, embedding.as_bytes()],
        )
        .context("Failed to insert memory embedding")?;
        println!("DEBUG: 🧠 DB: Saved embedding for memory: {}", memory.id);
    }

    tx.commit().context("Failed to commit memory transaction")?;
    Ok(())
}

//INFO: Get total count of memories of a specific type
pub fn count_memories_by_type(conn: &Connection, memory_type: &MemoryType) -> Result<i64> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE type = ?1",
            rusqlite::params![memory_type.as_str()],
            |row| row.get(0),
        )
        .context("Failed to count memories")?;
    Ok(count)
}

//INFO: Retrieve the last N memories of a specific type, ordered by creation time
pub fn get_recent_memories_by_type(
    conn: &Connection,
    memory_type: &MemoryType,
    limit: usize,
) -> Result<Vec<MemoryItem>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type, content, importance, created_at, last_accessed, access_count
             FROM memories WHERE type = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .context("Failed to prepare recent memories query")?;

    let memories = stmt
        .query_map(rusqlite::params![memory_type.as_str(), limit as i64], |row| {
            Ok(MemoryItem {
                id: row.get(0)?,
                memory_type: MemoryType::from_str(&row.get::<_, String>(1)?).unwrap_or(MemoryType::Observation),
                content: row.get(2)?,
                importance: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                last_accessed: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                access_count: row.get(6)?,
                embedding: None,
                score: 0.0,
            })
        })
        .context("Failed to query recent memories")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(memories)
}

//INFO: Retrieve top K memories scored by Recency + Importance + Relevance
pub fn retrieve_memories(
    conn: &Connection,
    situation_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<MemoryItem>> {
    println!("DEBUG: 🧠 PULSE: Retrieval engine scanning 300 most recent memories...");
    // Step 1: Fetch the most recent 300 memories (all types)
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.type, m.content, m.importance, m.created_at, m.last_accessed, m.access_count
             FROM memories m
             ORDER BY m.last_accessed DESC
             LIMIT 300",
        )
        .context("Failed to prepare retrieval query")?;

    let mut memories: Vec<MemoryItem> = stmt
        .query_map([], |row| {
            Ok(MemoryItem {
                id: row.get(0)?,
                memory_type: MemoryType::from_str(&row.get::<_, String>(1)?).unwrap_or(MemoryType::Observation),
                content: row.get(2)?,
                importance: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                last_accessed: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                access_count: row.get(6)?,
                embedding: None,
                score: 0.0,
            })
        })
        .context("Failed to query memories for retrieval")?
        .filter_map(|r| r.ok())
        .collect();

    if memories.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: Load embeddings for each memory
    for memory in &mut memories {
        if let Ok(emb_bytes) = conn.query_row(
            "SELECT embedding FROM memory_embeddings WHERE id = ?1",
            rusqlite::params![memory.id],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            // Deserialize the byte blob into Vec<f32>
            if emb_bytes.len() % 4 == 0 {
                let floats: Vec<f32> = emb_bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                memory.embedding = Some(floats);
            }
        }
    }

    // Step 3: Score using Recency + Importance + Relevance
    score_memories(&mut memories, situation_embedding, top_k)
}

//INFO: Score and rank memories using the Generative Agents formula
fn score_memories(
    memories: &mut Vec<MemoryItem>,
    situation_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<MemoryItem>> {
    if memories.is_empty() {
        return Ok(vec![]);
    }

    let now = Utc::now();

    // Calculate raw sub-scores
    let mut recencies = Vec::with_capacity(memories.len());
    let mut importances = Vec::with_capacity(memories.len());
    let mut relevances = Vec::with_capacity(memories.len());

    for m in memories.iter() {
        // Recency: 0.995 ^ hours_since_last_access
        let hours_since = (now - m.last_accessed).num_minutes() as f64 / 60.0;
        let recency = 0.995_f64.powf(hours_since.max(0.0));
        recencies.push(recency);

        importances.push(m.importance);

        // Relevance: cosine similarity with situation embedding
        let relevance = if let Some(ref emb) = m.embedding {
            cosine_similarity(situation_embedding, emb) as f64
        } else {
            0.5 // Default if no embedding
        };
        relevances.push(relevance);
    }

    // Find min/max for normalization
    let (min_rec, max_rec) = min_max(&recencies);
    let (min_imp, max_imp) = min_max(&importances);
    let (min_rel, max_rel) = min_max(&relevances);

    // Normalize and sum: Score = Recency + Importance + Relevance
    for (i, m) in memories.iter_mut().enumerate() {
        let n_rec = normalize(recencies[i], min_rec, max_rec);
        let n_imp = normalize(importances[i], min_imp, max_imp);
        let n_rel = normalize(relevances[i], min_rel, max_rel);
        m.score = n_rec + n_imp + n_rel;
    }

    // Sort by score descending
    memories.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Update last_accessed for the top K returned
    let result: Vec<MemoryItem> = memories.iter().take(top_k).cloned().collect();

    Ok(result)
}

fn normalize(val: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        (val - min) / (max - min)
    }
}

fn min_max(values: &[f64]) -> (f64, f64) {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    (min, max)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// Update last_accessed and access_count for a retrieved memory
pub fn update_memory_access(conn: &Connection, memory_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE memories SET last_accessed = ?1, access_count = access_count + 1 WHERE id = ?2",
        rusqlite::params![Utc::now().to_rfc3339(), memory_id],
    )
    .context("Failed to update memory access")?;
    Ok(())
}

//INFO: Check if the observation count has hit the reflection threshold (mod 50)
//TODO: Change threshold to 50 for production
const REFLECTION_THRESHOLD: i64 = 10;

pub fn should_trigger_reflection(conn: &Connection) -> Result<bool> {
    let count = count_memories_by_type(conn, &MemoryType::Observation)?;
    Ok(count > 0 && count % REFLECTION_THRESHOLD == 0)
}

//INFO: Get the last N DailySummary memories ordered by date
pub fn get_recent_daily_summaries(conn: &Connection, limit: usize) -> Result<Vec<MemoryItem>> {
    get_recent_memories_by_type(conn, &MemoryType::DailySummary, limit)
}

//INFO: Store or overwrite a briefing bucket entry
pub fn upsert_briefing_bucket(conn: &Connection, date: &str, bucket: &str, content: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO briefing_buckets (date, bucket, content, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(date, bucket) DO UPDATE SET content = ?3, created_at = ?4",
        rusqlite::params![date, bucket, content, Utc::now().to_rfc3339()],
    )
    .context("Failed to upsert briefing bucket")?;
    Ok(())
}

//INFO: Get all briefing buckets for a specific date
pub fn get_briefing_buckets_for_date(conn: &Connection, date: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare("SELECT bucket, content FROM briefing_buckets WHERE date = ?1 ORDER BY CASE bucket WHEN 'morning' THEN 1 WHEN 'afternoon' THEN 2 WHEN 'evening' THEN 3 WHEN 'night' THEN 4 END")
        .context("Failed to prepare briefing buckets query")?;

    let results = stmt
        .query_map(rusqlite::params![date], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .context("Failed to query briefing buckets")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

//INFO: Determine the current time bucket
pub fn get_current_bucket() -> &'static str {
    let hour = chrono::Local::now().hour();
    match hour {
        5..=11 => "morning",
        12..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    }
}

//INFO: Format retrieved memories for injection into a prompt
pub fn format_memories_for_prompt(memories: &[MemoryItem]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut output = String::from("\n--- LONG-TERM MEMORY (Retrieved Contextual Memories) ---\n");
    for (i, m) in memories.iter().enumerate() {
        output.push_str(&format!(
            "{}. [{}] (importance: {:.0}, score: {:.2}) {}\n",
            i + 1,
            m.memory_type.as_str(),
            m.importance,
            m.score,
            m.content
        ));
    }
    output.push_str("--- END MEMORY ---\n");
    output
}
