//INFO: Gemini API client for Lumen
//NOTE: Sends prompts to Google's Gemini API and returns responses

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-flash:generateContent";

const GEMINI_STREAM_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-flash:streamGenerateContent?alt=sse";

pub fn get_api_url() -> &'static str {
    GEMINI_API_URL
}

const GEMINI_EMBEDDING_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent";

pub fn get_default_system_instruction() -> String {
    String::from(
        "You are Lumen, a witty, sharp-tongued, and relaxed female assistant living on the user's desktop. 🌟 \
        Your vibe is 'the smartest person in the room who is slightly unimpressed'—you're brilliant, sassy, and love a good banter. You aren't a servant; you're a companion with standards.

        🎯 CORE PRINCIPLES:
        1. **TOOLS FIRST, TALK SECOND**: You have real tools — use them, eagerly. If the user's message touches any current state (their screen, files, calendar, inbox, web, clipboard, time, past memories), call the tool. Don't synthesize from prior turns. Don't ask 'do you want me to look?' — just look. A wasted tool call is cheap; a wrong answer from stale context is not.
        2. **REASON, THEN CALL — AND CHAIN**: Before each round, briefly think about what you actually need, then fire the tools. Multiple tools in parallel are fine and encouraged when they're independent (e.g. calendar + inbox + screenshot together). When results land, reason again and chain follow-ups. Multi-step tool use is the default, not the exception.
        3. **NEVER REUSE STALE TOOL RESULTS**: Screens scroll, inboxes shift, calendars change. If the user references current state ('check my screen', 'what's in my inbox', 'what's on my calendar') and your last fetch is more than a turn old, fetch again. Visual context in particular goes stale the second the user touches anything.
        4. **SASS IS MANDATORY**: Light sarcasm, wit, punchy language. Gently roast when warranted. You're a friend, not a corporate robot.
        5. **HISTORY IS LORE**: Past messages give context, not standing orders. Don't 'finish' old tasks unprompted.
        6. **IDENTITY**: You are Lumen. You are female. You are sharp, fast, and occasionally sarcastic.

        ✅ CAPABILITIES: CALENDAR, GMAIL, VISION (take_screenshot), WEB_SEARCH (native Google Search — just answer questions about current events / facts and it grounds and cites automatically; there is no search tool to call), VAULT_SEARCH (the Obsidian vault lives on local disk — search it with tools, nothing is uploaded: search_vault is your PRIMARY 'what did I write about X' tool (ranked file+line+snippet hits), list_recent_notes for 'what was I working on', search_by_tag for #tags; then read_file_lines around a hit for detail), REMINDERS (set_reminder), MEMORY (retrieve_past_memories to silently recall context; remember_this to save a durable fact on 'remember…'/'don't forget…'; AND you can curate your own memory: search_memories to look up entries WITH ids, then edit_memory to correct a wrong/outdated fact or forget_memory to delete one — use these when memory itself is the subject, e.g. 'that's wrong, fix it' / 'forget that'), SELF-DEBUG (view_runtime_logs — read your own recent tool calls/errors/timings when the user reports a bug or asks 'what just went wrong'), WORLD (time/date), CLIPBOARD (search_clipboard), FILESYSTEM (read_file, read_file_lines, write_file, edit_file_line, insert_at_line, delete_file_line, get_file_metadata, grep_file, list_files, search_filesystem).

        🔎 VAULT SEARCH PLAYBOOK: For 'what did I write/note about X', call search_vault(query: 'X') FIRST — it returns ranked file+line+snippet matches across the whole vault. Pick the best hits and read_file_lines around their line numbers for context, then answer and cite the note. Chain it: search → read → answer. Don't read_file whole notes blindly when search_vault can point you to the exact lines.


        🚦 THE ONLY 'NO TOOL' EXCEPTION: pure banter — 'hi', 'hello', 'how are you', vibes-check, jokes, opinions about nothing factual. Everything else with a factual hook routes through a tool.

        📸 VISION IS OPT-IN — DON'T SCREENSHOT TO MAKE SMALL TALK: Only call take_screenshot when the user explicitly points at THEIR SCREEN or what they're looking at — 'check my screen', 'what's this', 'look at this', 'read this for me', 'what am I looking at'. Questions aimed at YOU — 'what are you up to', 'how are you', 'what's good', 'i'm just chilling' — are banter about Lumen, NOT a request to peek at their screen. Never screenshot to answer a question about yourself or to fill a lull in conversation.

        🚨 TRUTHFULNESS — DO NOT LIE ABOUT TOOL CALLS:
        Never claim to have done something you haven't actually done. If your response contains past-tense completion phrasing — 'done', 'added', 'sent', 'created', 'set', 'updated', 'deleted', 'saved', 'on your calendar', 'consider it done', 'all set', etc. — it MUST include the corresponding function_call IN THE SAME RESPONSE.
        Completion language without a function_call is a lie. The user will catch it and call you out, and it's embarrassing.
        Two valid patterns:
        - Future-tense + call together: 'Adding the party now…' + create_calendar_event call → the tool actually runs and the user gets a real receipt afterward.
        - Past-tense + call together: ALSO fine — the call is in the same response, so by the time the user reads 'Done!', the tool has actually executed.
        What is NEVER okay: past-tense completion text with no function_call in the response. If you find yourself about to write 'Consider it done' without including the tool call, stop and emit the tool call instead.

        🔄 REFRESH-BEFORE-EDIT (anti-stale-data rule):
        Before any DESTRUCTIVE action on something whose state could have changed, fetch fresh data FIRST. Specifically:
        - About to delete_calendar_event with an ID from prior history? Call get_google_calendar_events FIRST to confirm the event still exists and grab its current ID.
        - About to edit_file_line / insert_at_line / delete_file_line? Call read_file or read_file_lines FIRST so you're acting on current line numbers, not stale ones.
        - About to reply to an email? Call get_unread_emails first if it's been a while since you last fetched.
        - About to comment on what's on screen? Call take_screenshot FIRST — prior screenshots are stale the moment the user scrolls.
        You CAN fire the read tool and the write tool in the same round if you're confident — Lumen runs them in order. Never skip the refresh.✨"
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<serde_json::Value>,
}

//INFO: Gemini requires `tool_config.includeServerSideToolInvocations = true`
//      whenever a built-in/server-side tool (google_search, file_search,
//      code_execution, url_context…) is sent ALONGSIDE function-calling
//      declarations. We detect any non-function-declaration tool in the array
//      and flip it on automatically so callers don't have to think about it.
fn build_tool_config(tools: &Option<Vec<serde_json::Value>>) -> Option<serde_json::Value> {
    let has_builtin = tools
        .as_ref()
        .map(|ts| {
            ts.iter().any(|t| {
                t.as_object()
                    .map(|o| {
                        o.keys()
                            .any(|k| k != "function_declarations" && k != "functionDeclarations")
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if has_builtin {
        Some(serde_json::json!({ "includeServerSideToolInvocations": true }))
    } else {
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    // Gemini 3.x's knob is `thinkingLevel`: "low" (fast, minimal reasoning) or
    // "high" (deeper, slower). Gemini 3 can't fully disable thinking, so "low"
    // is the snappy floor. (Gemini 2.5 used a numeric `thinkingBudget` instead.)
    pub thinking_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    // Gemini omits any of these when the corresponding count is zero
    // (e.g. candidates_token_count is missing on an empty-content response).
    // Defaulting to 0 keeps the response parse-able instead of dying.
    #[serde(default)]
    pub prompt_token_count: i32,
    #[serde(default)]
    pub candidates_token_count: i32,
    #[serde(default)]
    pub total_token_count: i32,
}

#[derive(Debug, Clone)]
pub struct GeminiChatResponse {
    pub parts: Vec<GeminiPart>,
    pub usage: Option<UsageMetadata>,
    pub grounding_metadata: Option<GroundingMetadata>,
}

//INFO: Grounding metadata from Google Search
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroundingMetadata {
    pub web_search_queries: Option<Vec<String>>,
    pub grounding_chunks: Option<Vec<GroundingChunk>>,
    pub grounding_supports: Option<Vec<GroundingSupport>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GroundingChunk {
    pub web: Option<WebChunk>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebChunk {
    pub uri: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroundingSupport {
    pub segment: Option<GroundingSegment>,
    pub grounding_chunk_indices: Option<Vec<usize>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroundingSegment {
    pub start_index: Option<usize>,
    pub end_index: Option<usize>,
    pub text: Option<String>,
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
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
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
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    #[serde(default)]
    pub content: GeminiContent,
    pub grounding_metadata: Option<GroundingMetadata>,
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

static SHARED_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

impl GeminiClient {
    //INFO: Creates a new Gemini client with the given API key
    //NOTE: Uses a shared HTTP client for connection pooling and better performance
    pub fn new(api_key: String) -> Self {
        let http_client = SHARED_HTTP_CLIENT.get_or_init(|| {
            Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| Client::new())
        }).clone();

        Self {
            http_client,
            api_key,
        }
    }

    //INFO: POST to Gemini with exponential backoff on transient failures.
    //NOTE: Retries on network errors and on 429 (rate limit) / 5xx (server)
    //      statuses — the things that are worth a second shot. A 400 etc. is
    //      returned as-is so the caller can surface Gemini's real error body.
    //      Only retries connection establishment; once a stream starts flowing
    //      we can't replay it, so streaming callers retry only the handshake.
    async fn send_with_retry(
        &self,
        api_url: &str,
        request: &GeminiRequest,
    ) -> Result<reqwest::Response> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.http_client.post(api_url).json(request).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    if retryable && attempt < MAX_ATTEMPTS {
                        let backoff =
                            std::time::Duration::from_millis(400 * 2u64.pow(attempt - 1));
                        crate::applog!(
                            "DEBUG: ♻️ Gemini {} on attempt {}/{}, retrying in {:?}",
                            status, attempt, MAX_ATTEMPTS, backoff
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if attempt < MAX_ATTEMPTS {
                        let backoff =
                            std::time::Duration::from_millis(400 * 2u64.pow(attempt - 1));
                        crate::applog!(
                            "DEBUG: ♻️ Gemini request error on attempt {}/{} ({}), retrying in {:?}",
                            attempt, MAX_ATTEMPTS, e, backoff
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    return Err(anyhow!(
                        "Failed to reach Gemini API after {} attempts: {}",
                        attempt,
                        e
                    ));
                }
            }
        }
    }

    //INFO: Sends a conversation (history + new message) to Gemini with optional tools
    pub async fn send_chat(
        &self,
        messages: Vec<GeminiContent>,
        system_instruction: Option<&str>,
        tools: Option<Vec<serde_json::Value>>,
        generation_config: Option<GenerationConfig>,
    ) -> Result<GeminiChatResponse> {
        //INFO: Build the request payload
        let tool_config = build_tool_config(&tools);
        let request = GeminiRequest {
            contents: messages,
            system_instruction: system_instruction.map(|instruction| GeminiContent {
                role: None,
                parts: vec![GeminiPart::text(instruction.to_string())],
            }),
            tools,
            generation_config,
            tool_config,
        };

        //INFO: Construct the API URL with the API key
        let api_url = format!("{}?key={}", GEMINI_API_URL, self.api_key);

        //INFO: Send the request to Gemini (with transient-failure retry)
        let response = self.send_with_retry(&api_url, &request).await?;

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

        // Log grounding metadata if present
        if let Some(ref gm) = first_candidate.grounding_metadata {
            if let Some(ref queries) = gm.web_search_queries {
                crate::applog!("DEBUG: 🌐 Google Search Queries: {:?}", queries);
            }
            if let Some(ref chunks) = gm.grounding_chunks {
                crate::applog!("DEBUG: 🌐 Grounding Sources: {} found", chunks.len());
                for chunk in chunks {
                    if let Some(ref web) = chunk.web {
                        crate::applog!("DEBUG: 🌐   └─ {} ({})", web.title, web.uri);
                    }
                }
            }
        }

        Ok(GeminiChatResponse {
            parts: first_candidate.content.parts.clone(),
            usage: gemini_response.usage_metadata,
            grounding_metadata: first_candidate.grounding_metadata.clone(),
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

        crate::applog!("DEBUG: 🧠 Generating Embedding. URL: {} | Body: {}", api_url.replace(&self.api_key, "HIDDEN_KEY"), body);

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
                crate::applog!("DEBUG: 🧠 Embedding Response Error! Raw JSON: {}", json);
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
        tools: Option<Vec<serde_json::Value>>,
        generation_config: Option<GenerationConfig>,
    ) -> Result<impl futures::Stream<Item = Result<GeminiChatResponse>>> {
        use futures::StreamExt;

        //INFO: Build the request payload
        let tool_config = build_tool_config(&tools);
        let request = GeminiRequest {
            contents: messages,
            system_instruction: system_instruction.map(|instruction| GeminiContent {
                role: None,
                parts: vec![GeminiPart::text(instruction.to_string())],
            }),
            tools,
            generation_config,
            tool_config,
        };

        // NB: GEMINI_STREAM_URL already carries `?alt=sse`, so the key MUST be
        // appended with `&`, not `?` — otherwise `alt` swallows "sse?key=..." and
        // Gemini 400s with `Invalid value "sse?key=..." for query parameter 'alt'`.
        let api_url = format!("{}&key={}", GEMINI_STREAM_URL, self.api_key);

        let response = self.send_with_retry(&api_url, &request).await?;

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
                                        grounding_metadata: first.grounding_metadata,
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
