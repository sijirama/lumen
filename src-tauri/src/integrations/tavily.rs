use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use crate::database::Database;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
    answer: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TavilyResult {
    pub title: String,
    pub url: String,
    pub content: String,
}

pub async fn search(db: &Database, query: &str) -> Result<serde_json::Value> {
    if query.is_empty() {
        return Ok(json!({ "error": "Query is empty" }));
    }

    println!("DEBUG: 🌐 Tavily Research Start: \"{}\"", query);

    // 1. Generate Query Hash (SHA256)
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    let query_hash = format!("{:x}", hasher.finalize());

    // 2. Check Cache
    {
        let connection = db.connection.lock();
        if let Ok(Some(results_json)) = crate::database::queries::get_web_search_cache(&connection, &query_hash) {
            println!("DEBUG: 🌐 Search Cache Hit!");
            return Ok(serde_json::from_str(&results_json)?);
        }
    }

    // 3. Get and Decrypt API Key
    let api_key = {
        let connection = db.connection.lock();
        let encrypted_key = crate::database::queries::get_api_token(&connection, "tavily")?
            .ok_or_else(|| anyhow!("Tavily API key not found in database. Please add it in Settings."))?;
        
        crate::crypto::decrypt_token(&encrypted_key)
            .map_err(|e| anyhow!("Failed to decrypt Tavily API key: {}", e))?
    };

    // 4. Request
    let client = crate::integrations::http::shared_client();
    let response = client
        .post("https://api.tavily.com/search")
        .json(&json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "advanced",
            "include_answer": true,
            "max_results": 5
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        println!("DEBUG: 🌐 Tavily API ERROR ({}): {}", status, text);
        return Err(anyhow!("Tavily API Error ({}): {}", status, text));
    }

    let tavily_res: TavilyResponse = response.json().await?;
    println!("DEBUG: 🌐 Tavily Results: {} found", tavily_res.results.len());
    
    let result = json!({
        "status": "success",
        "results": tavily_res.results,
        "quick_answer": tavily_res.answer,
    });

    // 5. Save to Cache
    {
        let connection = db.connection.lock();
        let result_json = serde_json::to_string(&result)?;
        let _ = crate::database::queries::save_web_search_cache(&connection, &query_hash, query, &result_json);
    }

    Ok(result)
}
