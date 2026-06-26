//INFO: Chat commands for Lumen
//NOTE: Handles AI chat functionality with Gemini

use crate::crypto::decrypt_token;
use crate::database::queries::{
    get_api_token, get_chat_messages, get_integration,
    get_user_profile, save_chat_message, ChatMessage, Citation, ToolInvocation,
};
use crate::database::Database;
use crate::gemini::{client::get_default_system_instruction, GeminiClient};
use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::State;

//INFO: Shared cancellation flag for the in-flight chat request. Set by the
//      `cancel_chat` command, checked between tool rounds in send_chat_message.
//      Only one chat runs at a time from the overlay so a single global flag
//      is enough — keeps the wiring trivial.
static CHAT_CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

//INFO: Command invoked by the Stop button. The currently-running chat loop
//      will notice the flag and bail out at the next round boundary.
#[tauri::command]
pub fn cancel_chat() {
    CHAT_CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
}

//INFO: Chat message for frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessageResponse {
    pub id: Option<i64>,
    pub role: String,
    pub content: String,
    pub image_data: Option<String>,
    pub created_at: String,
    pub citations: Option<Vec<Citation>>,
    pub tool_invocations: Option<Vec<ToolInvocation>>,
}


//INFO: Request to send a chat message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
    pub session_id: Option<String>,
    pub base64_image: Option<String>,
}

//INFO: Response from sending a chat message
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub user_message: ChatMessageResponse,
    pub assistant_message: ChatMessageResponse,
}

//INFO: Turn the raw accumulated model text into what the user actually sees.
//      Each `\n\n`-separated chunk is treated independently:
//        - prose chunks pass through unchanged
//        - stray JSON chunks are reduced to their `response` field if present,
//          else dropped (defensive — structured output was removed, but this
//          keeps any accidental JSON blob from leaking into the bubble)
//      Every round's prose survives into the final display.
fn render_text_for_display(raw: &str) -> String {
    raw.split("\n\n")
        .filter_map(|chunk| {
            let trimmed = chunk.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                return json_val
                    .get("response")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
            Some(chunk.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

//INFO: A terminal reply that's empty OR a tiny all-digit/punctuation blob ("00",
//      "0", "..") is the model failing, not answering — treat it as "no reply"
//      so the safety-net retry fires instead of saving junk.
fn looks_like_garbage(text: &str) -> bool {
    let t = text.trim();
    t.is_empty()
        || (t.chars().count() <= 2
            && t.chars().all(|c| c.is_numeric() || c.is_ascii_punctuation()))
}

//INFO: Maps a tool name to a friendly, present-tense status shown in the UI
//      while it runs (e.g. "📅 Checking your calendar…"). Unknown tools fall
//      back to a humanized version of their name so nothing ever shows raw.
fn tool_status_label(name: &str) -> String {
    let label = match name {
        "take_screenshot" => "📸 Looking at your screen",
        "retrieve_past_memories" => "🧠 Searching my memory",
        "search_memories" => "🧠 Searching my memory",
        "remember_this" => "🧠 Committing that to memory",
        "edit_memory" => "🧠 Updating a memory",
        "forget_memory" => "🧠 Forgetting that",
        "view_runtime_logs" => "🔧 Checking my own logs",
        "set_reminder" => "⏰ Setting a reminder",
        "search_clipboard" => "📋 Checking your clipboard",
        "get_weather" => "🌤️ Checking the weather",
        "get_unread_emails" => "📥 Reading your inbox",
        "send_email" => "✉️ Sending your email",
        "create_calendar_event" => "📅 Adding to your calendar",
        "delete_calendar_event" => "📅 Updating your calendar",
        "get_google_calendar_events" => "📅 Checking your calendar",
        "search_notes" | "search_filesystem" | "list_files" => "🔍 Searching your files",
        "get_obsidian_vault_info" => "🗂️ Reading your vault",
        "grep_file" | "read_file" | "read_file_lines" | "get_file_metadata" => "📄 Reading your files",
        "write_file" | "edit_file_line" | "insert_at_line" | "delete_file_line" => "✏️ Editing your files",
        other => return format!("⚙️ Running {}", other.replace('_', " ")),
    };
    label.to_string()
}

//INFO: Renders a past turn's tool calls + results into a compact text block so
//      the model can reference them on later turns (cross-turn tool memory).
//      Each result is stripped of heavy fields (screenshot base64) and truncated
//      so replayed history can't blow up the context window. Total per-turn block
//      is capped too, in case a single turn fired many tools.
fn render_tool_trace_for_history(invocations: &[ToolInvocation]) -> String {
    const MAX_RESULT_CHARS: usize = 1000;
    const MAX_BLOCK_CHARS: usize = 4000;

    let mut out = String::from(
        "\n\n⟦tool results from this turn — reference these if the user follows up⟧",
    );
    for inv in invocations {
        // Drop heavy/irrelevant fields (e.g. take_screenshot's base64 image).
        let mut result = inv.result.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.remove("image_data");
        }
        let mut result_str = result.to_string();
        if result_str.chars().count() > MAX_RESULT_CHARS {
            result_str = format!(
                "{}… (truncated)",
                result_str.chars().take(MAX_RESULT_CHARS).collect::<String>()
            );
        }
        let status = if inv.succeeded { "" } else { " [failed]" };
        out.push_str(&format!("\n• {}{} → {}", inv.name, status, result_str));

        if out.chars().count() > MAX_BLOCK_CHARS {
            out.push_str("\n• …(more tools omitted)");
            break;
        }
    }
    out
}

//INFO: Cap a tool result before it's fed back to the model. A tool result is
//      replayed into context on EVERY subsequent round of the same turn, so a
//      large one (read_file allows up to 500KB) would multiply token cost and
//      latency. Oversized results are replaced with a truncated preview plus a
//      hint to fetch a narrower slice.
fn truncate_tool_result(value: serde_json::Value) -> serde_json::Value {
    const MAX_CHARS: usize = 8000;
    let s = value.to_string();
    if s.chars().count() <= MAX_CHARS {
        return value;
    }
    let preview: String = s.chars().take(MAX_CHARS).collect();
    serde_json::json!({
        "note": "Result truncated — it was too large to include in full. If you need more, fetch a narrower slice (e.g. read_file_lines for a specific range, or a more specific search).",
        "truncated_preview": preview
    })
}

//INFO: Sends a message to the AI and returns the response
#[tauri::command]
pub async fn send_chat_message(
    app_handle: tauri::AppHandle,
    database: State<'_, Database>,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
    use tauri::Emitter;
    use futures::StreamExt;

    // Reset cancellation flag — this turn starts fresh.
    CHAT_CANCELLED.store(false, std::sync::atomic::Ordering::SeqCst);

    //INFO: Get the Gemini API key from the database
    let api_key = {
        let connection = database.connection.lock();
        let encrypted_key = get_api_token(&connection, "gemini")
            .map_err(|e| format!("Failed to get API key: {}", e))?
            .ok_or_else(|| {
                "Gemini API key not configured. Please add your API key in Settings.".to_string()
            })?;

        decrypt_token(&encrypted_key).map_err(|e| format!("Failed to decrypt API key: {}", e))?
    };

    //INFO: 1. Get Conversation History (Sliding Window: last 15 messages)
    let history = {
        let connection = database.connection.lock();
        get_chat_messages(&connection, request.session_id.as_deref(), 15)
            .map_err(|e| format!("Failed to get history: {}", e))?
    };

    //INFO: 2. Build context from integrations
    let context = build_chat_context(&database)?;

    //INFO: 3. Convert history to Gemini format (History is already chronological).
    //      For past ASSISTANT turns we replay a compact, truncated record of the
    //      tools that ran + what they returned, embedded as text in that turn.
    //      This is what gives Lumen cross-turn tool memory: ask "reply to the 2nd
    //      email" a turn after get_unread_emails and the data is still in context.
    //      We replay as TEXT (not real function_call/function_response parts) on
    //      purpose — it sidesteps Gemini 3.x's thought_signature requirements on
    //      replayed calls, and the truncation keeps old results from ballooning
    //      the window.
    let mut gemini_messages = Vec::new();
    for msg in history {
        let is_user = msg.role == "user";
        let mut content = msg.content;
        if !is_user {
            if let Some(invocations) = msg.tool_invocations.as_ref() {
                if !invocations.is_empty() {
                    content.push_str(&render_tool_trace_for_history(invocations));
                }
            }
        }
        gemini_messages.push(crate::gemini::client::GeminiContent {
            role: Some(if is_user { "user".to_string() } else { "model".to_string() }),
            parts: vec![crate::gemini::client::GeminiPart::text(content)],
        });
    }

    //INFO: 4. Add current message
    let mut parts = vec![crate::gemini::client::GeminiPart::text(request.message.clone())];

    if let Some(ref b64) = request.base64_image {
        parts.push(crate::gemini::client::GeminiPart::inline_data(
            "image/png".to_string(),
            b64.clone(),
        ));
    }

    gemini_messages.push(crate::gemini::client::GeminiContent {
        role: Some("user".to_string()),
        parts,
    });

    //INFO: 5. Load Tools based on active integrations
    let (google_enabled, obsidian_enabled) = {
        let connection = database.connection.lock();
        let g = crate::database::queries::get_integration(&connection, "google")
            .ok().flatten().map(|i| i.enabled).unwrap_or(false);
        let o = crate::database::queries::get_integration(&connection, "obsidian")
            .ok().flatten().map(|i| i.enabled).unwrap_or(false);
        (g, o)
    };
    let function_tool_declarations = crate::gemini::tools::build_tools_for_request(google_enabled, obsidian_enabled);
    let mut tools_json: Vec<serde_json::Value> = function_tool_declarations
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect();

    //INFO: Native Google Search grounding. Gemini runs the search itself, inline
    //      during generation, and returns grounding_metadata (parsed into
    //      citations below) — no function round-trip. This replaces the Tavily
    //      `search_web` tool. If a model ever 400s on google_search alongside
    //      function tools, removing this single push reverts to no web search.
    tools_json.push(serde_json::json!({ "google_search": {} }));

    let tools = tools_json;

    let obsidian_config = {
        let connection = database.connection.lock();
        get_integration(&connection, "obsidian")
            .ok()
            .flatten()
            .and_then(|i| i.config)
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
    };

    //INFO: 6. Send to Gemini (with Tool Loop)
    let client = GeminiClient::new(api_key.clone());

    //INFO: Enhance system instruction with specific user info
    let mut system_instruction = get_default_system_instruction();

    if let Some(ctx) = &context {
        system_instruction.push_str("\n\n--- CURRENT DIGITAL STATE (BACKGROUND CONTEXT) ---");
        system_instruction.push_str(
            "\nThis is a snapshot of the user's system at the start of the turn. Treat it as hints, not ground truth — if the request touches state that could have changed (screen content, files, inbox, calendar), still call the tool. Don't blather about this context unprompted.",
        );
        system_instruction.push_str(&format!("\n\n{}", ctx));
        system_instruction.push_str("\n-------------------------------------------");
    }

    //INFO: Memory retrieval is now opt-in via the `retrieve_past_memories` tool.
    //      The previous auto-embed-on-every-turn added a full embedding API
    //      round-trip to every chat turn — removed for latency and cost.

    //INFO: Dynamic CONFIRMATION RULE — built from the per-tool approval
    //      settings the user toggled in Settings. If they've turned approval
    //      OFF for create_calendar_event, it doesn't appear in this list and
    //      Lumen runs it without asking. If everything is OFF, the whole
    //      rule is skipped.
    {
        // (tool_name, default_when_unset, prompt_phrase)
        let approval_tools: &[(&str, bool, &str)] = &[
            ("send_email",             true,  "send_email → show recipient, subject, and the full body, then ask 'send it?'"),
            ("delete_calendar_event",  true,  "delete_calendar_event → name the event and time, then ask 'delete it?'"),
            ("write_file",             true,  "write_file → show the path and the content (or a clear summary if huge), then ask 'write it?'"),
            ("create_calendar_event",  false, "create_calendar_event → show summary/time/location, then ask 'create it?'"),
            ("edit_file_line",         true,  "edit_file_line → show the path, line number, and new content, then ask 'edit it?'"),
            ("insert_at_line",         true,  "insert_at_line → show the path, line number, and content to insert, then ask 'insert it?'"),
            ("delete_file_line",       true,  "delete_file_line → show the path and line number, then ask 'delete it?'"),
        ];

        let active_lines: Vec<&str> = {
            let connection = database.connection.lock();
            approval_tools
                .iter()
                .filter(|(name, default, _)| {
                    let key = format!("approval_{}", name);
                    match crate::database::queries::get_setting(&connection, &key) {
                        Ok(Some(val)) => val == "true",
                        _ => *default,
                    }
                })
                .map(|(_, _, phrase)| *phrase)
                .collect()
        };

        if !active_lines.is_empty() {
            system_instruction.push_str("\n\n⚠️ CONFIRMATION RULE (destructive / outbound actions):\n");
            system_instruction.push_str("Before invoking any of these tools, you MUST summarise what you're about to do in chat and wait for an explicit 'yes', 'go', 'do it', or similar. Only then call the tool.\n");
            for line in &active_lines {
                system_instruction.push_str(&format!("- {}\n", line));
            }
            system_instruction.push_str("Tools NOT in this list — including read-only tools (get_*, list_*, search_*, take_screenshot, retrieve_past_memories) — DO NOT need confirmation, just run them.\n");
            system_instruction.push_str("If the user already said 'go send X to Y' with all the details, you have your confirmation — execute. Don't bug them twice.\n");
            system_instruction.push_str("⚡ ONCE CONFIRMED, EXECUTE — DO NOT NARRATE: When the user says 'yes', 'go', 'do it', 'yes yes' etc, your VERY NEXT response MUST contain the function_call. Do NOT first send a 'Consider it done!' or 'On it!' message — that's a lie because the tool hasn't run yet. The confirmation phase is OVER. Send the function_call now.");
        }
    }

    if let Some(config) = &obsidian_config {
        system_instruction.push_str("\n\n--- OBSIDIAN CONFIGURATION ---");
        if let Some(path) = config.get("vault_path").and_then(|v| v.as_str()) {
            system_instruction.push_str(&format!("\nVault path: {}", path));
        }
        if let Some(folder) = config.get("daily_notes_path").and_then(|v| v.as_str()) {
            if !folder.is_empty() {
                system_instruction.push_str(&format!(
                    "\nDaily Notes folder (relative to vault): {}",
                    folder
                ));
            }
        }
        if let Some(format) = config.get("daily_notes_format").and_then(|v| v.as_str()) {
            system_instruction.push_str(&format!(
                "\nDaily Notes date format (Moment.js syntax): {}",
                format
            ));
        }
        system_instruction.push_str("\n------------------------------");
    }

    //INFO: Smart auto-retrieval. The `retrieve_past_memories` tool is still there,
    //      but the model rarely calls it, so memories never surfaced. Here we
    //      proactively inject relevant memories — but ONLY when the message looks
    //      personal/past-referencing, so ordinary turns don't pay for an embedding
    //      round-trip. One embed call → vec0 KNN → top 5 into context.
    {
        let msg_lc = request.message.to_lowercase();
        const MEMORY_HINTS: &[&str] = &[
            "remember", "you said", "i told you", "i mentioned", "last time",
            "we talked", "we discussed", "recall", "what did i", "did i tell",
            "know about me", "my favorite", "my favourite", "as i said",
            "like i said", "i already told", "used to", "back then",
        ];
        if MEMORY_HINTS.iter().any(|h| msg_lc.contains(h)) {
            match client.generate_embedding(&request.message).await {
                Ok(embedding) => {
                    let block = {
                        let connection = database.connection.lock();
                        match crate::memory::core::retrieve_memories(&connection, &embedding, 5) {
                            Ok(mems) if !mems.is_empty() => {
                                for m in &mems {
                                    let _ = crate::memory::core::update_memory_access(&connection, &m.id);
                                }
                                Some(crate::memory::core::format_memories_for_prompt(&mems))
                            }
                            _ => None,
                        }
                    };
                    if let Some(block) = block {
                        crate::applog!("DEBUG: 🧠 Auto-retrieval injected memories into context.");
                        system_instruction.push_str(&block);
                    }
                }
                Err(e) => crate::applog!("DEBUG: 🧠 Auto-retrieval embed failed: {}", e),
            }
        }
    }

    let history_count = gemini_messages.len();

    let tool_count: usize = tools.iter().map(|t| {
        if let Some(funcs) = t.get("function_declarations").and_then(|f| f.as_array()) {
            funcs.len()
        } else if t.get("google_search").is_some() {
            1
        } else {
            0
        }
    }).sum();

    //INFO: PROMPT BILL OF MATERIALS (For Speed Audit)
    crate::applog!("\n--- 📦 PROMPT BILL OF MATERIALS ---");
    crate::applog!("├─ 📜 System Instruction: {} chars", system_instruction.len());
    if let Some(ctx) = &context {
        crate::applog!("├─ 🔍 Context: {} chars (Snippet: {}...)", ctx.len(), &ctx[..100.min(ctx.len())].replace("\n", " "));
    } else {
        crate::applog!("├─ 🔍 Context: NONE");
    }
    crate::applog!("├─ 🕒 History: {} messages", history_count);
    crate::applog!("├─ 🔧 Tools: {} available", tool_count);
    crate::applog!("------------------------------------\n");
    let mut current_messages = gemini_messages;
    let mut final_response_text = String::new();
    let mut final_grounding_metadata: Option<crate::gemini::client::GroundingMetadata> = None;
    // Accumulates the full audit trail of tool calls across all rounds so the user
    // can see (and inspect) what Lumen actually did to produce the final reply.
    let mut tool_invocations: Vec<ToolInvocation> = Vec::new();


    //INFO: Tool execution loop — uses non-streaming for tool rounds
    //NOTE: Only the FINAL response (no function calls) gets streamed to the UI
    crate::applog!("DEBUG: 🤖 Using Gemini API at: {}", crate::gemini::client::get_api_url());

    // Reasoning depth, adaptive. "low" keeps simple turns (banter, calendar,
    // email) snappy — it was the biggest silent latency tax when always on. BUT
    // low thinking + an image makes gemini-3.5-flash produce degenerate replies
    // (empty / "00"), so vision turns get "high": we start high if the user
    // attached an image, and bump to high the moment a screenshot enters the turn
    // (see the attachment handling below). Config is rebuilt per round from this
    // flag, so the bump takes effect on the very next round.
    let mut high_thinking = request.base64_image.is_some();
    let mut tool_round_config = crate::gemini::client::GenerationConfig {
        max_output_tokens: Some(2048),
        thinking_config: Some(crate::gemini::client::ThinkingConfig {
            thinking_level: if high_thinking { "high" } else { "low" }.to_string(),
        }),
        ..Default::default()
    };

    //INFO: Per-tool call counter to prevent runaway tool loops.
    //      Worst-case budget: MAX_TOOL_ROUNDS × (tools per round) ≈ deep research.
    //      Latency is the trade-off — each round is one Gemini API call.
    let mut tool_call_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    const MAX_CALLS_PER_TOOL: usize = 8;
    const MAX_TOOL_ROUNDS: usize = 10;

    //INFO: Hard wall-clock budget for the whole turn. MAX_TOOL_ROUNDS alone could
    //      stack up to 10 × (per-call timeout) of dead air; this caps the total so
    //      a stuck or pathologically chatty turn bails gracefully instead of
    //      spinning forever. On expiry we break and let the synthesis/fallback
    //      below produce a reply from whatever we've gathered.
    let turn_start = std::time::Instant::now();
    const TURN_BUDGET_SECS: u64 = 90;

    //INFO: Code-level vision gate. take_screenshot is only legitimate when THIS
    //      message references the screen. The system prompt says so, but a
    //      low-thinking model still over-fires it on banter ("what are you up
    //      to"), so we enforce it in the harness too — belt and suspenders.
    let user_msg_lc = request.message.to_lowercase();
    const SCREEN_HINTS: &[&str] = &[
        "screen", "screenshot", "look at", "looking at", "see this", "see that",
        "what's this", "whats this", "what is this", "read this", "on my display",
        "this page", "what do you see", "can you see", "my desktop", "what am i",
        "this window", "show you", "peek at", "on screen",
    ];
    let mentions_screen = SCREEN_HINTS.iter().any(|h| user_msg_lc.contains(h));

    for _i in 0..MAX_TOOL_ROUNDS {
        // Cancellation check at round boundary — user clicked Stop.
        if CHAT_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
            crate::applog!("DEBUG: 🛑 Chat cancelled by user before round {}", _i + 1);
            break;
        }

        // Wall-clock budget check at round boundary — bail gracefully if a turn
        // is dragging on, rather than letting it run all MAX_TOOL_ROUNDS.
        if turn_start.elapsed().as_secs() >= TURN_BUDGET_SECS {
            crate::applog!("DEBUG: ⏰ Turn exceeded {}s budget — bailing to synthesize from what we have.", TURN_BUDGET_SECS);
            break;
        }

        // DEBUG: Dump the full prompt state to disk. Opt-in via LUMEN_DEBUG_PROMPTS
        // so the synchronous fs::write (it serializes the entire history + every
        // tool schema, on every round) stays out of the normal hot path.
        if std::env::var("LUMEN_DEBUG_PROMPTS").is_ok() {
            log_prompt_debug(
                &request.message,
                &system_instruction,
                &current_messages,
                &tools,
                _i + 1,
            );
        }

        // Stream this round from Gemini (SSE). Under the tool-eager prompt a tool
        // round usually comes back as a pure function_call with no text, so nothing
        // surfaces to the UI until the terminal round — which streams its prose
        // token-by-token. Any preamble text the model emits ALONGSIDE a tool call
        // is wiped once we discover the call, so the user only ever reads the real
        // final answer (tools-first, talk-second).
        let round_start = std::time::Instant::now();
        let stream = client
            .stream_chat(
                current_messages.clone(),
                Some(&system_instruction),
                Some(tools.clone()),
                Some(tool_round_config.clone()),
            )
            .await
            .map_err(|e| format!("Failed to get AI response: {}", e))?;
        futures::pin_mut!(stream);

        // Accumulate the round as chunks land: prose into `round_text` (emitted
        // live), and every non-text part (function_call / thought) preserved
        // verbatim so the model turn we replay into history keeps its
        // thought_signatures intact for Gemini 2.5 function calling.
        let mut round_text = String::new();
        let mut non_text_parts: Vec<crate::gemini::client::GeminiPart> = Vec::new();
        let mut round_usage: Option<crate::gemini::client::UsageMetadata> = None;
        let mut streamed_prose = false;

        while let Some(chunk) = stream.next().await {
            // Mid-stream cancellation — honor Stop the moment it's clicked, not
            // just at round boundaries. Drops the SSE connection and keeps the
            // partial prose streamed so far.
            if CHAT_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
                crate::applog!("DEBUG: 🛑 Cancelled mid-stream.");
                break;
            }

            let chunk = chunk.map_err(|e| format!("Failed to get AI response: {}", e))?;

            if chunk.grounding_metadata.is_some() {
                final_grounding_metadata = chunk.grounding_metadata.clone();
            }
            if chunk.usage.is_some() {
                round_usage = chunk.usage.clone();
            }

            for part in chunk.parts {
                // Gemini 3.x streams its reasoning as parts flagged `thought: true`
                // that ALSO carry `text`. That text is NOT the answer — it must
                // never reach the chat bubble. This was the screenshot "00"/bare-
                // number bug: vision turns force `thinking: high`, so a reasoning
                // fragment leaked through the `part.text` check below and showed as
                // the reply. Keep a thought part only if it carries a
                // thought_signature (needed for multi-turn function-call continuity);
                // otherwise drop it entirely. (function_call parts keep their own
                // signature via the `else` branch, so this doesn't affect them.)
                if part.thought.is_some() {
                    if part.thought_signature.is_some() {
                        non_text_parts.push(part);
                    }
                    continue;
                }
                if let Some(text) = &part.text {
                    round_text.push_str(text);
                    streamed_prose = true;
                    // Live token stream → UI. We emit the whole round-so-far; the
                    // frontend replaces the in-flight (id: -1) bubble wholesale.
                    let _ = app_handle.emit("assistant-reply-turn", round_text.clone());
                } else {
                    non_text_parts.push(part);
                }
            }
        }
        let llm_elapsed = round_start.elapsed();

        // If Stop was hit mid-stream, keep whatever prose streamed and bail before
        // executing any tools from this (incomplete) round.
        if CHAT_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
            crate::applog!("DEBUG: 🛑 Chat cancelled mid-stream — keeping partial reply, skipping tools.");
            if streamed_prose {
                if !final_response_text.is_empty() {
                    final_response_text.push_str("\n\n");
                }
                final_response_text.push_str(&round_text);
            }
            break;
        }

        if let Some(usage) = &round_usage {
            crate::applog!("DEBUG: ⚡ Round {} LLM call: {:.2?} | Tokens -> Prompt: {}, Candidates: {}, Total: {}", _i + 1, llm_elapsed, usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count);
        } else {
            crate::applog!("DEBUG: ⚡ Round {} LLM call: {:.2?}", _i + 1, llm_elapsed);
        }

        // Rebuild the model turn for history: merged prose + the preserved
        // non-text parts (function calls carrying their thought_signatures).
        let mut model_parts: Vec<crate::gemini::client::GeminiPart> = Vec::new();
        if !round_text.is_empty() {
            model_parts.push(crate::gemini::client::GeminiPart::text(round_text.clone()));
        }
        model_parts.extend(non_text_parts.iter().cloned());
        current_messages.push(crate::gemini::client::GeminiContent {
            role: Some("model".to_string()),
            parts: model_parts,
        });

        let mut has_function_calls = false;
        let mut function_responses = Vec::new();

        // Pass 1: collect tool calls (checking rate limits)
        let mut async_calls: Vec<crate::gemini::client::GeminiFunctionCall> = Vec::new();
        let mut sync_calls: Vec<crate::gemini::client::GeminiFunctionCall> = Vec::new();

        for part in &non_text_parts {
            if let Some(call) = &part.function_call {
                crate::applog!("DEBUG: 🛠️ Tool Call -> {} (args: {})", call.name, call.args);

                // Vision gate: refuse take_screenshot when the message never
                // referenced the screen. We feed a refusal back (has_function_calls
                // = true) instead of silently dropping it, so the model turn keeps a
                // matching function_response in history and the next round answers
                // the user directly.
                if call.name == "take_screenshot" && !mentions_screen {
                    crate::applog!("DEBUG: 🚫 take_screenshot blocked — message doesn't reference the screen.");
                    has_function_calls = true;
                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name.clone(),
                        serde_json::json!({ "error": "Screenshot unavailable: the user didn't reference their screen this turn. Answer them directly — do NOT screenshot." }),
                    ));
                    continue;
                }

                let count = tool_call_counts.entry(call.name.clone()).or_insert(0);
                *count += 1;

                if *count > MAX_CALLS_PER_TOOL {
                    crate::applog!("DEBUG: ⚠️ Tool '{}' hit call limit, skipping.", call.name);
                    function_responses.push(crate::gemini::client::GeminiPart::function_response(
                        call.name.clone(),
                        serde_json::json!({ "error": format!("Tool '{}' called too many times. Synthesize from what you have.", call.name) }),
                    ));
                } else {
                    has_function_calls = true;
                    if crate::gemini::tools::is_async(&call.name) {
                        async_calls.push(call.clone());
                    } else {
                        sync_calls.push(call.clone());
                    }
                }
            }
        }

        // If this round streamed preamble prose AND also wants tools, that prose
        // was just narration ("on it…") — wipe the in-flight bubble so the user
        // waits on a clean spinner and only reads the real answer streamed by the
        // terminal round.
        if has_function_calls && streamed_prose {
            let _ = app_handle.emit("assistant-reply-clear", ());
        }

        // The terminal round (no tool calls) is the actual answer — keep its prose.
        if !has_function_calls && streamed_prose {
            if !final_response_text.is_empty() {
                final_response_text.push_str("\n\n");
            }
            final_response_text.push_str(&round_text);
        }

        // Pass 2: execute — async tools in parallel, sync tools sequentially
        if has_function_calls {
            // Tell the UI WHAT we're doing, not just THAT we're busy. Dedup so two
            // file reads don't show "Reading your files" twice.
            let mut running_labels: Vec<String> = Vec::new();
            for c in async_calls.iter().chain(sync_calls.iter()) {
                let label = tool_status_label(&c.name);
                if !running_labels.contains(&label) {
                    running_labels.push(label);
                }
            }
            let _ = app_handle.emit("tool-execution-start", running_labels);

            // Async tools run concurrently. Per-tool wall-clock timing is logged
            // so we can see which integrations are dragging out chat turns.
            // Tuple shape: (name, args, result, duration_ms, started_at_rfc3339)
            let db_inner = database.inner().clone();
            let app_handle_inner = app_handle.clone();
            let async_results: Vec<(String, serde_json::Value, crate::gemini::tools::ToolResult, u64, String)> = futures::future::join_all(
                async_calls.into_iter().map(|call| {
                    let db = db_inner.clone();
                    let ah = app_handle_inner.clone();
                    async move {
                        // Per-tool timeout so one hung integration can't ride the
                        // whole turn budget. Underlying HTTP clients already cap
                        // network calls; this guards against any other stall.
                        const TOOL_TIMEOUT_SECS: u64 = 45;
                        let started_at = chrono::Utc::now().to_rfc3339();
                        let start = std::time::Instant::now();
                        let res = match tokio::time::timeout(
                            std::time::Duration::from_secs(TOOL_TIMEOUT_SECS),
                            crate::gemini::tools::execute_tool_async(&call.name, &call.args, &db, &ah),
                        ).await {
                            Ok(r) => r,
                            Err(_) => {
                                crate::applog!("DEBUG: ⏱️  Tool '{}' (async) TIMED OUT after {}s", call.name, TOOL_TIMEOUT_SECS);
                                crate::gemini::tools::ToolResult::ok(serde_json::json!({
                                    "error": format!("Tool '{}' timed out after {}s. Proceed without its result.", call.name, TOOL_TIMEOUT_SECS)
                                }))
                            }
                        };
                        let elapsed = start.elapsed();
                        crate::applog!("DEBUG: ⏱️  Tool '{}' (async) took {:.2?}", call.name, elapsed);
                        (call.name, call.args, res, elapsed.as_millis() as u64, started_at)
                    }
                })
            ).await;

            // Sync tools run sequentially (single DB lock for the batch)
            let sync_results: Vec<(String, serde_json::Value, crate::gemini::tools::ToolResult, u64, String)> = {
                let connection = database.connection.lock();
                sync_calls.into_iter().map(|call| {
                    let started_at = chrono::Utc::now().to_rfc3339();
                    let start = std::time::Instant::now();
                    let res = crate::gemini::tools::execute_tool_sync(
                        &call.name, &call.args,
                        obsidian_config.as_ref(), &connection, &app_handle,
                    );
                    let elapsed = start.elapsed();
                    crate::applog!("DEBUG: ⏱️  Tool '{}' (sync) took {:.2?}", call.name, elapsed);
                    (call.name, call.args, res, elapsed.as_millis() as u64, started_at)
                }).collect()
            };

            let _ = app_handle.emit("tool-execution-end", ());

            // Pass 3: process results, collect inline attachments, build function_responses
            let mut attachments: Vec<crate::gemini::tools::ToolAttachment> = Vec::new();
            for (name, args, res, duration_ms, started_at) in async_results.into_iter().chain(sync_results.into_iter()) {
                crate::applog!("DEBUG: ✅ Tool '{}' Result: {}", name, res.response);

                if name == "create_calendar_event" || name == "delete_calendar_event" {
                    if res.response.get("status").and_then(|s| s.as_str()) == Some("success") || res.response.get("events").is_some() {
                        let _ = app_handle.emit("calendar-updated", ());
                    }
                }

                // Record this call for the persisted trace. Success = no top-level "error" key.
                let succeeded = res.response.get("error").is_none();
                tool_invocations.push(ToolInvocation {
                    name: name.clone(),
                    args,
                    result: res.response.clone(),
                    duration_ms,
                    started_at,
                    succeeded,
                });

                if let Some(att) = res.attachment {
                    attachments.push(att);
                }

                // Strip any fields that exist only for the persisted trace, not for
                // the model. Right now this is just take_screenshot's `image_data` —
                // it's already going to Gemini as a proper `inline_data` part, and
                // including the b64 inside the function_response would double-count
                // the tokens (and easily blow past Gemini's 1M-token input cap on
                // any reasonably-sized screenshot).
                let response_for_model = if name == "take_screenshot" {
                    let mut cleaned = res.response.clone();
                    if let Some(obj) = cleaned.as_object_mut() {
                        obj.remove("image_data");
                    }
                    cleaned
                } else {
                    res.response
                };
                // Cap oversized results (e.g. a 500KB read_file) before they re-enter
                // context EVERY subsequent round of this turn — protects tokens/latency.
                let response_for_model = truncate_tool_result(response_for_model);
                function_responses.push(crate::gemini::client::GeminiPart::function_response(name, response_for_model));
            }

            // NOTE: we deliberately DO NOT emit `assistant-reply-clear` here.
            // Prose from this round needs to stay visible while the next round
            // runs more tools and appends its own prose.

            // Merge function responses AND any binary attachments (e.g. screenshots)
            // into a SINGLE user turn. Two consecutive user turns confuse Gemini —
            // it tends to STOP with empty content. One turn with mixed parts works.
            let mut combined_parts = function_responses;
            if !attachments.is_empty() {
                // Vision just entered the turn — bump reasoning to "high" so the
                // model actually reads the image and follows through on multi-step
                // requests instead of emitting junk like "00".
                if !high_thinking {
                    high_thinking = true;
                    tool_round_config.thinking_config = Some(crate::gemini::client::ThinkingConfig {
                        thinking_level: "high".to_string(),
                    });
                    crate::applog!("DEBUG: 🧠 Vision in play — bumping thinking to 'high' for synthesis.");
                }
                combined_parts.push(crate::gemini::client::GeminiPart::text(
                    "[VISUAL CONTEXT ATTACHED]".to_string(),
                ));
                for att in attachments {
                    combined_parts.push(crate::gemini::client::GeminiPart::inline_data(att.mime_type, att.data));
                }
            }
            current_messages.push(crate::gemini::client::GeminiContent {
                role: Some("user".to_string()),
                parts: combined_parts,
            });
            continue;
        } else {
            break;
        }
    }

    // If the user cancelled and we have no real text, drop in a short
    // placeholder so the chat doesn't look broken.
    let was_cancelled = CHAT_CANCELLED.load(std::sync::atomic::Ordering::SeqCst);
    if was_cancelled && looks_like_garbage(&render_text_for_display(&final_response_text)) {
        final_response_text = "_(stopped)_".to_string();
    }

    //INFO: Safety net — the model used tools but never produced a USABLE reply.
    //      We check the RENDERED, trimmed text (not raw `.is_empty()`) so a model
    //      that ends a tool turn with a stray whitespace/1-token response — which
    //      is exactly what produced the "tool chip, no message" bug — still
    //      triggers a real retry. Force one more call WITHOUT tools and WITHOUT a
    //      JSON schema so it MUST just talk, and stream it like any other answer.
    if looks_like_garbage(&render_text_for_display(&final_response_text)) && !was_cancelled {
        crate::applog!("DEBUG: ⚠️ No usable text after tool loop. Forcing a final text-only call...");

        // Throw away whatever whitespace junk accumulated and clear the dead
        // in-flight bubble so the retry streams into a clean slate.
        final_response_text.clear();
        let _ = app_handle.emit("assistant-reply-clear", ());

        let forced_stream = client
            .stream_chat(
                current_messages.clone(),
                Some(&system_instruction),
                None, // No tools — forces a pure text response
                Some(tool_round_config.clone()),
            )
            .await
            .map_err(|e| format!("Failed to get forced response: {}", e))?;
        futures::pin_mut!(forced_stream);

        while let Some(chunk) = forced_stream.next().await {
            if CHAT_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
                crate::applog!("DEBUG: 🛑 Cancelled mid-stream (forced fallback).");
                break;
            }
            let chunk = chunk.map_err(|e| format!("Failed to get forced response: {}", e))?;
            for part in chunk.parts {
                // Same thought-leak guard as the main loop — never stream reasoning.
                if part.thought.is_some() {
                    continue;
                }
                if let Some(text) = &part.text {
                    final_response_text.push_str(text);
                    let _ = app_handle.emit("assistant-reply-turn", final_response_text.clone());
                }
            }
        }

        if looks_like_garbage(&render_text_for_display(&final_response_text)) {
            return Err("Lumen processed the request but couldn't generate a response. Please try again.".to_string());
        }
    }

    //INFO: Save both messages to the database
    let now = chrono::Utc::now().to_rfc3339();

    let user_message = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: request.message.clone(),
        image_data: request.base64_image.clone(),
        citations: None,
        tool_invocations: None,
        created_at: now.clone(),
        session_id: request.session_id.clone(),
    };

    //INFO: Build the user-visible message from the full accumulated text.
    let actual_final_text = render_text_for_display(&final_response_text);

    //INFO: Citations come from native Google Search grounding metadata.
    let mut citations: Option<Vec<Citation>> = None;
    if let Some(ref gm) = final_grounding_metadata {
        if let Some(ref chunks) = gm.grounding_chunks {
            let citation_list: Vec<Citation> = chunks
                .iter()
                .filter_map(|chunk| {
                    chunk.web.as_ref().map(|web| Citation {
                        title: web.title.clone(),
                        url: web.uri.clone(),
                    })
                })
                .collect();
            if !citation_list.is_empty() {
                crate::applog!("DEBUG: 🌐 Extracted {} citations from native grounding.", citation_list.len());
                citations = Some(citation_list);
            }
        }
    }

    let persisted_invocations = if tool_invocations.is_empty() {
        None
    } else {
        Some(tool_invocations.clone())
    };

    let assistant_message = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: actual_final_text.clone(),
        image_data: None,
        citations: citations.clone(),
        tool_invocations: persisted_invocations.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        session_id: request.session_id.clone(),
    };

    //INFO: Save messages to database
    let (user_id, assistant_id) = {
        let connection = database.connection.lock();
        let user_id = save_chat_message(&connection, &user_message)
            .map_err(|e| format!("Failed to save user message: {}", e))?;
        let assistant_id = save_chat_message(&connection, &assistant_message)
            .map_err(|e| format!("Failed to save assistant message: {}", e))?;

        // Set session title from first user message (only sets if title is NULL)
        if let Some(ref sid) = request.session_id {
            let title = request.message.chars().take(60).collect::<String>();
            let _ = crate::database::queries::update_session_title(&connection, sid, &title);
        }

        (user_id, assistant_id)
    };

    //INFO: Memory Extraction Trigger. Two knobs, deliberately decoupled:
    //        CADENCE — how often extraction fires (every N messages). Kept
    //                  moderate so Lumen keeps learning regularly, not in rare
    //                  bursts. Must be even (each turn saves 2 messages).
    //        WINDOW  — how many recent messages the extractor actually reads.
    //                  Bumped way up (was 6) so each pass has rich context to
    //                  synthesize dense, high-quality memories from. The window
    //                  overlaps successive passes; the near-duplicate check in
    //                  the background task drops any memories that repeat.
    const MEMORY_EXTRACTION_CADENCE: i64 = 20;
    const MEMORY_EXTRACTION_WINDOW: i64 = 50;
    {
        let connection = database.connection.lock();
        if let Ok(total_count) = crate::database::queries::count_chat_messages(&connection) {
            crate::applog!("DEBUG: 🧠 PULSE: Current chat message count: {}. (Cadence: {}, Window: {})", total_count, MEMORY_EXTRACTION_CADENCE, MEMORY_EXTRACTION_WINDOW);
            if total_count > 0 && total_count % MEMORY_EXTRACTION_CADENCE == 0 {
                crate::applog!("DEBUG: 🧠 TRIGGER: Memory extraction cadence hit! Initializing background task...");

                // Grab the last WINDOW messages for extraction
                let mut recent_messages = crate::database::queries::get_chat_messages(
                    &connection,
                    None,
                    MEMORY_EXTRACTION_WINDOW as i32,
                ).unwrap_or_default();
                
                // Reverse to chronological order (get_chat_messages returns newest first)
                recent_messages.reverse();

                let messages_for_extraction: Vec<String> = recent_messages
                    .iter()
                    .map(|m| format!("{}: {}", if m.role == "user" { "Sijibomi" } else { "Lumen" }, m.content))
                    .collect();

                // Clone what we need for the background task
                let db_clone = database.inner().clone();
                let api_key_clone = api_key.clone();
                // Fire and forget - async background extraction
                tokio::spawn(async move {
                    crate::applog!("DEBUG: 🧠 Starting background memory extraction...");

                    let user_name = {
                        let conn = db_clone.connection.lock();
                        crate::database::queries::get_user_profile(&conn)
                            .ok()
                            .flatten()
                            .map(|p| p.display_name)
                            .unwrap_or_else(|| "User".to_string())
                    };

                    let prompt = crate::memory::extractor::build_chat_extraction_prompt(&messages_for_extraction, &user_name);
                    let client = GeminiClient::new(api_key_clone.clone());

                    // Ask Gemini to extract memories
                    let extraction_result = client.send_chat(
                        vec![crate::gemini::client::GeminiContent {
                            role: Some("user".to_string()),
                            parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                        }],
                        Some("You are a memory extraction agent. Return ONLY valid JSON arrays."),
                        None,
                        Some(crate::gemini::client::GenerationConfig {
                            response_mime_type: Some("application/json".to_string()),
                            response_schema: None,
                            ..Default::default()
                        }),
                    ).await;

                    match extraction_result {
                        Ok(chat_response) => {
                            if let Some(usage) = &chat_response.usage {
                                crate::applog!("DEBUG: 🧠 Extraction Token Usage -> Prompt: {}, Candidates: {}, Total: {}", usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count);
                            }
                            let response_text = chat_response.parts.iter()
                                .filter(|p| p.thought.is_none())
                                .filter_map(|p| p.text.as_ref())
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("");

                            match crate::memory::extractor::parse_extracted_memories(&response_text) {
                                Ok(mut memories) => {
                                    crate::applog!("DEBUG: 🧠 Extracted {} memories from chat!", memories.len());
                                    for memory in &mut memories {
                                        // Generate embedding for each memory
                                        match client.generate_embedding(&memory.content).await {
                                            Ok(embedding) => {
                                                memory.embedding = Some(embedding);
                                                crate::applog!("DEBUG: 🧠 [{}] (importance: {}) {}", 
                                                    memory.memory_type.as_str(),
                                                    memory.importance,
                                                    memory.content.chars().take(80).collect::<String>()
                                                );
                                            }
                                            Err(e) => {
                                                crate::applog!("DEBUG: 🧠 Failed to embed memory: {}", e);
                                            }
                                        }
                                        
                                        // Store in DB with deduplication check
                                        let conn = db_clone.connection.lock();
                                        if let Some(ref emb) = memory.embedding {
                                            if crate::memory::core::is_near_duplicate(&conn, emb, 0.92) {
                                                crate::applog!("DEBUG: 🧠 Skipping near-duplicate memory.");
                                                continue;
                                            }
                                        }
                                        if let Err(e) = crate::memory::core::store_memory(&conn, memory) {
                                            crate::applog!("DEBUG: 🧠 Failed to store memory: {}", e);
                                        }
                                    }
                                    crate::applog!("DEBUG: 🧠 Memory extraction complete! ✅");

                                    // Check if we should trigger a Reflection loop
                                    {
                                        let conn = db_clone.connection.lock();
                                        match crate::memory::core::should_trigger_reflection(&conn) {
                                            Ok(true) => {
                                                crate::applog!("DEBUG: 🧠 Reflection threshold hit! Starting synthesis...");
                                                // Feed the reflection the recent observation window the prompt
                                                // promises (~50), NOT the 6-message extraction batch size.
                                                if let Ok(recent_obs) = crate::memory::core::get_recent_memories_by_type(
                                                    &conn,
                                                    &crate::memory::core::MemoryType::Observation,
                                                    crate::memory::core::REFLECTION_THRESHOLD as usize,
                                                ) {
                                                    let obs_texts: Vec<String> = recent_obs.iter().map(|o| o.content.clone()).collect();
                                                    let user_name = {
                                                        let conn = db_clone.connection.lock();
                                                        crate::database::queries::get_user_profile(&conn)
                                                            .ok()
                                                            .flatten()
                                                            .map(|p| p.display_name)
                                                            .unwrap_or_else(|| "User".to_string())
                                                    };
                                                    let prompt = crate::memory::reflection::build_reflection_prompt(&obs_texts, &user_name);
                                                    
                                                    // Drop lock for async synthesis
                                                    drop(conn);
                                                    
                                                    let api_key_reflection = api_key_clone.clone();
                                                    tokio::spawn(async move {
                                                        let client = GeminiClient::new(api_key_reflection);
                                                        crate::applog!("DEBUG: 🧠 Requesting reflection from Gemini...");
                                                        
                                                        let synthesis_result = client.send_chat(
                                                            vec![crate::gemini::client::GeminiContent {
                                                                role: Some("user".to_string()),
                                                                parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                                                            }],
                                                            Some("You are a reflection agent. Return ONLY a JSON array of reflections."),
                                                            None,
                                                            Some(crate::gemini::client::GenerationConfig {
                                                                response_mime_type: Some("application/json".to_string()),
                                                                response_schema: None,
                                                                ..Default::default()
                                                            }),
                                                        ).await;

                                                        if let Ok(resp) = synthesis_result {
                                                            let text = resp.parts.iter().filter(|p| p.thought.is_none()).filter_map(|p| p.text.as_ref()).cloned().collect::<Vec<_>>().join("");
                                                            if let Ok(reflections) = serde_json::from_str::<Vec<crate::memory::reflection::ExtractedReflection>>(&text) {
                                                                crate::applog!("DEBUG: 🧠 Synthesized {} high-level reflections!", reflections.len());
                                                                for r in reflections {
                                                                    let mut memory = crate::memory::extractor::create_memory(
                                                                        crate::memory::core::MemoryType::Reflection,
                                                                        r.content,
                                                                        r.importance
                                                                    );
                                                                    
                                                                    // Embed and store
                                                                    if let Ok(emb) = client.generate_embedding(&memory.content).await {
                                                                        memory.embedding = Some(emb);
                                                                        let conn = db_clone.connection.lock();
                                                                        let _ = crate::memory::core::store_memory(&conn, &memory);
                                                                        let reflection_snippet = memory.content.chars().take(60).collect::<String>();
                                                                        crate::applog!("DEBUG: 🧠 Stored reflection: {}", reflection_snippet);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                            Ok(false) => {}
                                            Err(e) => crate::applog!("DEBUG: 🧠 Reflection check failed: {}", e),
                                        }
                                    }
                                }
                                Err(e) => {
                                    crate::applog!("DEBUG: 🧠 Failed to parse extracted memories: {}", e);
                                    crate::applog!("DEBUG: 🧠 Raw response: {}", response_text);
                                }
                            }
                        }
                        Err(e) => {
                            crate::applog!("DEBUG: 🧠 Memory extraction LLM call failed: {}", e);
                        }
                    }
                });
            }
        }
    }

    Ok(SendMessageResponse {
        user_message: ChatMessageResponse {
            id: Some(user_id),
            role: user_message.role,
            content: user_message.content,
            image_data: user_message.image_data,
            created_at: user_message.created_at,
            citations: None,
            tool_invocations: None,
        },
        assistant_message: ChatMessageResponse {
            id: Some(assistant_id),
            role: assistant_message.role,
            content: actual_final_text,
            image_data: None,
            created_at: assistant_message.created_at,
            citations,
            tool_invocations: persisted_invocations,
        },
    })
}

//INFO: Gets chat history
#[tauri::command]
pub fn get_chat_history(
    database: State<Database>,
    session_id: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<ChatMessageResponse>, String> {
    let connection = database.connection.lock();
    let limit = limit.unwrap_or(50);

    let messages = get_chat_messages(&connection, session_id.as_deref(), limit)
        .map_err(|e| format!("Failed to get chat history: {}", e))?;

    Ok(messages
        .into_iter()
        .map(|m| ChatMessageResponse {
            id: m.id,
            role: m.role,
            content: m.content,
            image_data: m.image_data,
            created_at: m.created_at,
            citations: m.citations,
            tool_invocations: m.tool_invocations,
        })
        .collect())
}

//INFO: Clears all chat history (optionally for a specific session), with background summary generation
#[tauri::command]
pub async fn clear_chat_history(
    database: State<'_, Database>,
    session_id: Option<String>,
) -> Result<(), String> {
    use crate::database::queries::{clear_chat_messages, get_chat_messages};

    // Grab recent messages for summary before clearing
    let (messages_for_summary, api_key_opt, session_id_for_summary) = {
        let conn = database.connection.lock();
        let msgs = get_chat_messages(&conn, session_id.as_deref(), 30).unwrap_or_default();
        let key = crate::database::queries::get_api_token(&conn, "gemini")
            .ok()
            .flatten()
            .and_then(|enc| crate::crypto::decrypt_token(&enc).ok());
        (msgs, key, session_id.clone())
    };

    // Clear the messages
    {
        let conn = database.connection.lock();
        if let Some(ref sid) = session_id {
            conn.execute(
                "DELETE FROM chat_messages WHERE session_id = ?",
                rusqlite::params![sid],
            )
            .map_err(|e| format!("Failed to clear session messages: {}", e))?;
        } else {
            clear_chat_messages(&conn).map_err(|e| format!("Failed to clear chat history: {}", e))?;
        }
    }

    // Background: generate summary and store as memory
    if !messages_for_summary.is_empty() {
        if let Some(api_key) = api_key_opt {
            let db_clone = database.inner().clone();
            tokio::spawn(async move {
                let transcript: Vec<String> = messages_for_summary
                    .iter()
                    .map(|m| format!("{}: {}", if m.role == "user" { "User" } else { "Lumen" }, m.content))
                    .collect();
                let prompt = format!(
                    "Summarize this conversation in 2-3 sentences, capturing the key topics, decisions, and outcomes:\n\n{}",
                    transcript.join("\n")
                );
                let client = crate::gemini::GeminiClient::new(api_key.clone());
                if let Ok(resp) = client.send_chat(
                    vec![crate::gemini::client::GeminiContent {
                        role: Some("user".to_string()),
                        parts: vec![crate::gemini::client::GeminiPart::text(prompt)],
                    }],
                    Some("You are a concise summarizer. Return only the summary text, no preamble."),
                    None,
                    None,
                ).await {
                    let summary = resp.parts.iter().filter(|p| p.thought.is_none()).filter_map(|p| p.text.as_ref()).cloned().collect::<Vec<_>>().join("");
                    if !summary.is_empty() {
                        // Store as a memory
                        let mut memory = crate::memory::extractor::create_memory(
                            crate::memory::core::MemoryType::Observation,
                            format!("Conversation summary: {}", summary),
                            6.0,
                        );
                        if let Ok(emb) = client.generate_embedding(&memory.content).await {
                            memory.embedding = Some(emb);
                        }
                        let conn = db_clone.connection.lock();
                        let _ = crate::memory::core::store_memory(&conn, &memory);

                        // Also store in session record if we have a session_id
                        if let Some(ref sid) = session_id_for_summary {
                            let _ = crate::database::queries::update_session_summary(&conn, sid, &summary);
                        }
                        crate::applog!("DEBUG: 🧠 Session summary stored as memory.");
                    }
                }
            });
        }
    }

    Ok(())
}

//INFO: Creates a new chat session and returns its ID
#[tauri::command]
pub fn create_new_session(database: State<Database>) -> Result<String, String> {
    let connection = database.connection.lock();
    crate::database::queries::create_session(&connection).map_err(|e| e.to_string())
}

//INFO: Returns all chat sessions ordered by most recent
#[tauri::command]
pub fn get_chat_sessions(database: State<Database>) -> Result<Vec<crate::database::queries::ChatSession>, String> {
    let connection = database.connection.lock();
    crate::database::queries::get_sessions(&connection).map_err(|e| e.to_string())
}

//INFO: Helper to log prompt state for debugging
fn log_prompt_debug(
    message: &str,
    system: &str,
    history: &[crate::gemini::client::GeminiContent],
    tools: &[serde_json::Value],
    round: usize,
) {
    use std::fs;
    use std::path::Path;

    // 1. Sanitize filename (take first 30 chars of user message)
    let safe_message: String = message
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .take(30)
        .collect::<String>()
        .trim()
        .replace(" ", "_");

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_R{}_{}.json", timestamp, round, safe_message);
    
    // Path: src-tauri/test_prompts/ (assuming we are in src-tauri or project root)
    let debug_dir = Path::new("test_prompts");

    if !debug_dir.exists() {
        let _ = fs::create_dir_all(debug_dir);
    }

    let debug_data = serde_json::json!({
        "round": round,
        "timestamp": timestamp,
        "user_message": message,
        "system_instruction": system,
        "history": history,
        "tools": tools,
    });

    if let Ok(json_str) = serde_json::to_string_pretty(&debug_data) {
        let _ = fs::write(debug_dir.join(filename), json_str);
    }
}

//INFO: Builds context string from integrations (calendar, notes, etc.)
fn build_chat_context(database: &State<Database>) -> Result<Option<String>, String> {
    let mut context_parts: Vec<String> = Vec::new();

    // 1. Static Metadata
    let today = Local::now();
    let today_str = today.format("%A, %b %d").to_string();
    let current_time = today.format("%H:%M").to_string();
    let iso_now = today.to_rfc3339();
    context_parts.push(format!("Today: {} at {}", today_str, current_time));

    // 2. Integration Data (Locked Section - Keep it brief)
    let (user_profile, g_int, o_int) = {
        let connection = database.connection.lock();
        let user_profile = get_user_profile(&connection).ok().flatten();
        let g_int = get_integration(&connection, "google").ok().flatten();
        let o_int = get_integration(&connection, "obsidian").ok().flatten();
        (user_profile, g_int, o_int)
    };

    if let Some(profile) = user_profile {
        context_parts.push(format!("User Name: {}", profile.display_name));
    }

    context_parts.push(format!("\n[TECHNICAL CONTEXT]\nISO_NOW: {}", iso_now));

    let mut status_parts = Vec::new();
    status_parts.push("--- INTEGRATION STATUS ---".to_string());
    status_parts.push(format!("Google Services: {}", if g_int.as_ref().is_some_and(|i| i.enabled) { "ENABLED" } else { "DISABLED" }));
    status_parts.push(format!("Obsidian: {}", if o_int.as_ref().is_some_and(|i| i.enabled) { "ENABLED" } else { "DISABLED" }));
    status_parts.push("--------------------------".to_string());
    context_parts.push(status_parts.join("\n"));

    // 3. Obsidian Data (NO LOCKS - Pure Disk I/O)
    if let Some(integration) = o_int {
        if integration.enabled {
            if let Some(config) = integration.config {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config) {
                    if let Some(vault_path) = config_json.get("vault_path").and_then(|v| v.as_str()) {
                        let daily_notes_folder = config_json.get("daily_notes_path").and_then(|v| v.as_str()).unwrap_or("");
                        let date_format_raw = config_json.get("daily_notes_format").and_then(|v| v.as_str()).unwrap_or("YYYY-MM-DD");

                        let chrono_format = date_format_raw.replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d");
                        let daily_note_name = format!("{}.md", today.format(&chrono_format));
                        let daily_note_path = std::path::Path::new(vault_path).join(daily_notes_folder).join(&daily_note_name);

                        if daily_note_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&daily_note_path) {
                                let truncated_content = if content.chars().count() > 2000 {
                                    format!("{}... (truncated)", content.chars().take(2000).collect::<String>())
                                } else {
                                    content
                                };
                                context_parts.push(format!(
                                    "Today's daily note (NAME: {}, PATH: {}):\n{}",
                                    daily_note_name,
                                    daily_note_path.to_string_lossy(),
                                    truncated_content
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if context_parts.len() > 1 {
        Ok(Some(context_parts.join("\n\n")))
    } else {
        Ok(None)
    }
}
