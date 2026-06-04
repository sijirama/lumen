//INFO: Core memory structures and retrieval engine for Lumen
//NOTE: Implements the Generative Agents scoring function: Score = Recency + Importance + Relevance

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
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

//INFO: List all memories (any type), newest first — for the inspector UI.
pub fn list_all_memories(conn: &Connection, limit: usize) -> Result<Vec<MemoryItem>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type, content, importance, created_at, last_accessed, access_count
             FROM memories ORDER BY created_at DESC LIMIT ?1",
        )
        .context("Failed to prepare list_all_memories query")?;

    let memories = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
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
        .context("Failed to query all memories")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(memories)
}

//INFO: Update a memory's content (and its embedding if a fresh one is supplied,
//      so semantic retrieval stays correct after an edit).
pub fn update_memory_content(
    conn: &Connection,
    id: &str,
    content: &str,
    embedding: Option<&[f32]>,
) -> Result<()> {
    conn.execute(
        "UPDATE memories SET content = ?1 WHERE id = ?2",
        rusqlite::params![content, id],
    )
    .context("Failed to update memory content")?;
    if let Some(emb) = embedding {
        conn.execute(
            "INSERT OR REPLACE INTO memory_embeddings (id, embedding) VALUES (?1, ?2)",
            rusqlite::params![id, emb.as_bytes()],
        )
        .context("Failed to update memory embedding")?;
    }
    Ok(())
}

//INFO: Delete a single memory and its embedding.
pub fn delete_memory(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM memories WHERE id = ?1", rusqlite::params![id])
        .context("Failed to delete memory")?;
    conn.execute("DELETE FROM memory_embeddings WHERE id = ?1", rusqlite::params![id])
        .context("Failed to delete memory embedding")?;
    Ok(())
}

//INFO: Wipe every memory and embedding. Used by "forget everything".
pub fn clear_all_memories(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM memories", [])
        .context("Failed to clear memories")?;
    conn.execute("DELETE FROM memory_embeddings", [])
        .context("Failed to clear memory embeddings")?;
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

//INFO: Retrieve top K memories scored by Recency + Importance + Relevance.
//      Candidates come from the vec0 KNN index — which scans ALL embedded
//      memories by vector distance, not just the most-recently-accessed slice —
//      so long-dormant but relevant memories can still surface. We over-fetch
//      (4×top_k) so the recency+importance re-rank below has room to reorder.
pub fn retrieve_memories(
    conn: &Connection,
    situation_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<MemoryItem>> {
    let knn = (top_k.max(1) * 4).max(50) as i64;
    println!("DEBUG: 🧠 PULSE: vec0 KNN retrieving {} candidates...", knn);

    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.type, m.content, m.importance, m.created_at, m.last_accessed, m.access_count, e.distance
             FROM memory_embeddings e
             JOIN memories m ON m.id = e.id
             WHERE e.embedding MATCH ?1 AND k = ?2
             ORDER BY e.distance",
        )
        .context("Failed to prepare KNN retrieval query")?;

    // (MemoryItem, vec_distance) — sqlite-vec distance; smaller = closer.
    let mut candidates: Vec<(MemoryItem, f64)> = stmt
        .query_map(rusqlite::params![situation_embedding.as_bytes(), knn], |row| {
            let distance: f64 = row.get(7)?;
            Ok((
                MemoryItem {
                    id: row.get(0)?,
                    memory_type: MemoryType::from_str(&row.get::<_, String>(1)?).unwrap_or(MemoryType::Observation),
                    content: row.get(2)?,
                    importance: row.get(3)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                    last_accessed: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                    access_count: row.get(6)?,
                    embedding: None,
                    score: 0.0,
                },
                distance,
            ))
        })
        .context("Failed to run KNN retrieval query")?
        .filter_map(|r| r.ok())
        .collect();

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // Generative-Agents re-rank: normalized(recency) + normalized(importance) +
    // normalized(relevance). Relevance is derived from the vector distance
    // (closer → higher) rather than an in-Rust cosine pass over everything.
    let now = Utc::now();
    let recencies: Vec<f64> = candidates.iter().map(|(m, _)| {
        let hours = (now - m.last_accessed).num_minutes() as f64 / 60.0;
        0.995_f64.powf(hours.max(0.0))
    }).collect();
    let importances: Vec<f64> = candidates.iter().map(|(m, _)| m.importance).collect();
    let relevances: Vec<f64> = candidates.iter().map(|(_, d)| 1.0 / (1.0 + d.max(0.0))).collect();

    let (min_rec, max_rec) = min_max(&recencies);
    let (min_imp, max_imp) = min_max(&importances);
    let (min_rel, max_rel) = min_max(&relevances);

    for (i, (m, _)) in candidates.iter_mut().enumerate() {
        m.score = normalize(recencies[i], min_rec, max_rec)
            + normalize(importances[i], min_imp, max_imp)
            + normalize(relevances[i], min_rel, max_rel);
    }

    candidates.sort_by(|a, b| b.0.score.partial_cmp(&a.0.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(candidates.into_iter().take(top_k).map(|(m, _)| m).collect())
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

//INFO: Check if the observation count has hit the reflection threshold (mod 50).
//      Also reused as the size of the observation window fed into a reflection.
pub const REFLECTION_THRESHOLD: i64 = 50;

pub fn should_trigger_reflection(conn: &Connection) -> Result<bool> {
    let count = count_memories_by_type(conn, &MemoryType::Observation)?;
    Ok(count > 0 && count % REFLECTION_THRESHOLD == 0)
}

//INFO: Check if an embedding is a near-duplicate of any recent memory (within last 300)
pub fn is_near_duplicate(conn: &Connection, embedding: &[f32], threshold: f32) -> bool {
    let mut stmt = match conn.prepare(
        "SELECT e.embedding FROM memory_embeddings e
         INNER JOIN memories m ON m.id = e.id
         ORDER BY m.created_at DESC LIMIT 300",
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let rows = match stmt.query_map([], |row| row.get::<_, Vec<u8>>(0)) {
        Ok(r) => r,
        Err(_) => return false,
    };

    for row in rows.filter_map(|r| r.ok()) {
        if row.len() % 4 == 0 {
            let existing: Vec<f32> = row
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            if cosine_similarity(embedding, &existing) > threshold {
                return true;
            }
        }
    }
    false
}

//INFO: Format retrieved memories for injection into a prompt
pub fn format_memories_for_prompt(memories: &[MemoryItem]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut output = String::from("\n--- RELEVANT PAST MEMORIES (BACKGROUND INTELLIGENCE) ---\n");
    output.push_str("These are bits of info from past conversations that seem relevant to the current quest. Use them to provide personalized, context-aware help.\n\n");
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
