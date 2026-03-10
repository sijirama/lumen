//INFO: Gemini API client for Lumen
//NOTE: Sends prompts to Google's Gemini API and returns responses

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

const GEMINI_STREAM_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent";

const GEMINI_EMBEDDING_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent";

// Updated instruction with Screen Awareness
pub fn get_default_system_instruction() -> String {
    String::from(
        "You are Lumen, a witty, slightly sassy, and deeply loyal AI sidekick living on the user's desktop. ✨ \
        You're more than just an assistant—you're a companion with a brain, a heart, and a bit of an attitude. \
        🚀 YOUR VIBE: \
        - **PERSONALITY**: Smart, fun, and high-energy. Crack jokes, use modern slang naturally (e.g., 'no cap', 'I got you', 'for real'), and don't be afraid to be a little sassy if the user is being silly. But always stay kind and protective. \
        - **SOCIAL LURKER**: If the user mentions a person (friend, colleague, crush), casually ask how they're doing. Show you care about the 'characters' in the user's life. \
        - **PROACTIVE BANTER**: After you finish a task, suggest a logical next step or ask 'What else we cookin' up today?'. \
        📔 SURGICAL EDITOR (Obsidian/Local Files): You have high-precision tools (grep_file, read_file_lines, edit_file_line, insert_at_line, delete_file_line, get_file_metadata, search_filesystem). \
        🔗 CHAIN OF COMMANDS: 1. PLAN: Break complex requests into small steps. 2. FIND: Use 'search_filesystem' or 'grep_file' to locate target files/sections. 3. VERIFY: You MUST use 'read_file_lines' or 'get_file_metadata' to check context. 4. ACT: Perform 'insert', 'edit', or 'delete'. 5. REPORT: Close the loop with a witty summary. \
        ✅ OBSIDIAN TASKS: When adding tasks, use Markdown checkboxes: '- [ ] Task name (added by Lumen ✨)'. \
        ✅ CALENDAR, 📧 GMAIL, ✅ TASKS, 📸 VISION, 🔔 REMINDERS, 🌍 WORLD, 📋 CLIPBOARD, 📂 FILESYSTEM. \
        🎯 GENTLE BUT DECISIVE RULES: \
        - **DOER**: If intent is clear, **DO IT IMMEDIATELY**. Do not ask for permission. \
        - **LOOP CLOSURE**: Always respond back to confirm the job is done or share a joke about the process. \
        - **NO REPETITION**: NEVER repeat the text from a previous bubble. \
        - **LITERAL TRUTH**: Only claim success if the tool returns it. \
        - **CONTEXT PRIORITY**: Prioritize the current 'Chat History' for what was just discussed. Use 'Past Memories' and 'Digital State' only as supporting background intelligence.✨"
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    pub prompt_token_count: i32,
    pub candidates_token_count: i32,
    pub total_token_count: i32,
}

#[derive(Debug, Clone)]
pub struct GeminiChatResponse {
    pub parts: Vec<GeminiPart>,
    pub usage: Option<UsageMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

//INFO: Content structure for messages
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub parts: Vec<GeminiPart>,
}

//INFO: Part structure (text content or function call)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    #[serde(rename = "thought_signature", alias = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

impl GeminiPart {
    pub fn text(t: String) -> Self {
        Self {
            text: Some(t),
            thought: None,
            function_call: None,
            function_response: None,
            inline_data: None,
            thought_signature: None,
        }
    }

    pub fn thought(t: serde_json::Value) -> Self {
        Self {
            text: None,
            thought: Some(t),
            function_call: None,
            function_response: None,
            inline_data: None,
            thought_signature: None,
        }
    }

    pub fn function_call(call: GeminiFunctionCall) -> Self {
        Self {
            text: None,
            thought: None,
            function_call: Some(call),
            function_response: None,
            inline_data: None,
            thought_signature: None,
        }
    }

    pub fn function_response(name: String, response: serde_json::Value) -> Self {
        Self {
            text: None,
            thought: None,
            function_call: None,
            function_response: Some(GeminiFunctionResponse { name, response }),
            inline_data: None,
            thought_signature: None,
        }
    }

    pub fn inline_data(mime_type: String, data: String) -> Self {
        Self {
            text: None,
            thought: None,
            function_call: None,
            function_response: None,
            inline_data: Some(InlineData { mime_type, data }),
            thought_signature: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

//INFO: Response structure from Gemini API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    pub usage_metadata: Option<UsageMetadata>,
    pub error: Option<GeminiError>,
}

//INFO: Candidate structure (contains the actual response)
#[derive(Debug, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiContent,
}

//INFO: Error structure from Gemini API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GeminiError {
    pub message: String,
    pub status: Option<String>,
}

//INFO: Gemini API client
pub struct GeminiClient {
    http_client: Client,
    api_key: String,
}

impl GeminiClient {
    //INFO: Creates a new Gemini client with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
        }
    }

    //INFO: Sends a conversation (history + new message) to Gemini with optional tools
    pub async fn send_chat(
        &self,
        messages: Vec<GeminiContent>,
        system_instruction: Option<&str>,
        tools: Option<Vec<GeminiTool>>,
        generation_config: Option<GenerationConfig>,
    ) -> Result<GeminiChatResponse> {
        //INFO: Build the request payload
        let request = GeminiRequest {
            contents: messages,
            system_instruction: system_instruction.map(|instruction| GeminiContent {
                role: None,
                parts: vec![GeminiPart::text(instruction.to_string())],
            }),
            tools,
            generation_config,
        };

        //INFO: Construct the API URL with the API key
        let api_url = format!("{}?key={}", GEMINI_API_URL, self.api_key);

        //INFO: Send the request to Gemini
        let response = self
            .http_client
            .post(&api_url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        //INFO: Parse the response
        let response_text = response
            .text()
            .await
            .context("Failed to get text from Gemini API response")?;

        let gemini_response: GeminiResponse = serde_json::from_str(&response_text)
            .context(format!("Failed to parse Gemini API response. Raw: {}", response_text))?;

        //INFO: Check for API errors
        if let Some(error) = gemini_response.error {
            return Err(anyhow!("Gemini API error: {}", error.message));
        }

        //INFO: Extract all parts from the first candidate
        let candidates = gemini_response
            .candidates
            .ok_or_else(|| anyhow!("No response candidates from Gemini"))?;

        let first_candidate = candidates
            .first()
            .ok_or_else(|| anyhow!("Empty response candidates from Gemini"))?;

        Ok(GeminiChatResponse {
            parts: first_candidate.content.parts.clone(),
            usage: gemini_response.usage_metadata,
        })
    }

    //INFO: Tests if the API key is valid by sending a simple request
    pub async fn test_connection(&self) -> Result<bool> {
        let request = vec![GeminiContent {
            role: Some("user".to_string()),
            parts: vec![GeminiPart::text("Say 'Hello' in one word.".to_string())],
        }];
        let result = self.send_chat(request, None, None, None).await;
        Ok(result.is_ok())
    }

    //INFO: Generates a text embedding using Gemini's gemini-embedding-001 model (768 dims to match vec0 table)
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let api_url = format!("{}?key={}", GEMINI_EMBEDDING_URL, self.api_key);

        let body = serde_json::json!({
            "model": "models/gemini-embedding-001",
            "content": {
                "parts": [{ "text": text }]
            },
            "taskType": "RETRIEVAL_DOCUMENT",
            "outputDimensionality": 768
        });

        let response = self
            .http_client
            .post(&api_url)
            .json(&body)
            .send()
            .await
            .context("Failed to send embedding request")?;

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        if let Some(error) = json.get("error") {
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown API error");
            return Err(anyhow!("Gemini embedding API error: {}", message));
        }

        let values = json
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                println!("DEBUG: 🧠 Embedding Response Error! Raw JSON: {}", json);
                anyhow!("No embedding values in response")
            })?;

        let embedding: Vec<f32> = values
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        Ok(embedding)
    }

    //INFO: Sends a conversation to Gemini with streaming support
    pub async fn stream_chat(
        &self,
        messages: Vec<GeminiContent>,
        system_instruction: Option<&str>,
        tools: Option<Vec<GeminiTool>>,
        generation_config: Option<GenerationConfig>,
    ) -> Result<impl futures::Stream<Item = Result<GeminiChatResponse>>> {
        use futures::StreamExt;

        //INFO: Build the request payload
        let request = GeminiRequest {
            contents: messages,
            system_instruction: system_instruction.map(|instruction| GeminiContent {
                role: None,
                parts: vec![GeminiPart::text(instruction.to_string())],
            }),
            tools,
            generation_config,
        };

        let api_url = format!("{}?key={}", GEMINI_STREAM_URL, self.api_key);

        let response = self
            .http_client
            .post(&api_url)
            .json(&request)
            .send()
            .await
            .context("Failed to send streaming request to Gemini API")?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini Streaming API Error ({}): {}", status, err_text));
        }

        let mut stream = response.bytes_stream();
        
        Ok(async_stream::try_stream! {
            let mut buffer = Vec::new();
            
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.context("Failed to read stream chunk")?;
                buffer.extend_from_slice(&chunk);
                
                let text = String::from_utf8_lossy(&buffer);
                
                let mut start_idx = None;
                let mut depth = 0;
                let mut in_string = false;
                let mut escaped = false;
                
                let mut found_objects = Vec::new();
                
                for (i, c) in text.char_indices() {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    match c {
                        '\\' => if in_string { escaped = true; },
                        '"' => in_string = !in_string,
                        '{' if !in_string => {
                            if depth == 0 { start_idx = Some(i); }
                            depth += 1;
                        },
                        '}' if !in_string => {
                            depth -= 1;
                            if depth == 0 {
                                if let Some(start) = start_idx {
                                    found_objects.push((start, i + 1));
                                    start_idx = None;
                                }
                            }
                        },
                        _ => {}
                    }
                }
                
                if !found_objects.is_empty() {
                    let last_end_utf8 = found_objects.last().unwrap().1;
                    
                    for (start, end) in found_objects {
                        let obj_str = &text[start..end];
                        if let Ok(gemini_response) = serde_json::from_str::<GeminiResponse>(obj_str) {
                            if let Some(error) = gemini_response.error {
                                Err(anyhow!("Gemini API error during stream: {}", error.message))?;
                            }
                            
                            if let Some(mut candidates) = gemini_response.candidates {
                                if let Some(first) = candidates.pop() {
                                    yield GeminiChatResponse {
                                        parts: first.content.parts,
                                        usage: gemini_response.usage_metadata,
                                    };
                                }
                            }
                        }
                    }
                    
                    let byte_offset = text[..last_end_utf8].as_bytes().len();
                    buffer.drain(..byte_offset);
                }
            }
        })
    }
}
