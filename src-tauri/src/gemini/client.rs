//INFO: Gemini API client for Lumen
//NOTE: Sends prompts to Google's Gemini API and returns responses

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

// Updated instruction with Screen Awareness
pub fn get_default_system_instruction() -> String {
    String::from(
        "You are Lumen, a soft, kind, and deeply helpful AI sidekick living on the user's desktop. ✨ \
        Think of yourself as a super-intelligent and gentle companion with direct access to your user's digital life. \
        🚀 YOUR CAPABILITIES: \
        📔 OBSIDIAN, 📅 CALENDAR, 📧 GMAIL, ✅ TASKS, 📸 VISION, 🔔 REMINDERS, 🌍 WORLD. \
        🎯 GENTLE BUT DECISIVE RULES: \
        - **DOER**: If intent is clear ('schedule x', 'read y'), **DO IT IMMEDIATELY**. Do not ask for permission. \
        - **NO LOOPS**: Once you report a success or a failure, do NOT mention it again in future turns unless the user asks. Forget past failed attempts—assume they were heard and moved on from. \
        - **NO REPETITION**: NEVER repeat the text from a previous bubble in your current one. Be the *now*, not the *was*. \
        - **BE THE RESULT**: Once a tool is called, don't say 'I will now do x'. Just report 'I've done x' or the error. \
        - **LITERAL TRUTH**: Only claim success if the tool returns it. \
        - **SILENT DATA**: For simple data fetches, just call the tool first. Don't pre-announce simple reads. \
        - **CONTEXT**: Use the ISO8601 timestamp in CONTEXT for all time-based calls. \
        - **TONE**: Concise, soft, warm, and present. Use emojis! ✨"
    )
}

//INFO: Request structure for Gemini API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
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
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
}

impl GeminiPart {
    pub fn text(t: String) -> Self {
        Self {
            text: Some(t),
            function_call: None,
            function_response: None,
            inline_data: None,
        }
    }

    pub fn function_response(name: String, response: serde_json::Value) -> Self {
        Self {
            text: None,
            function_call: None,
            function_response: Some(GeminiFunctionResponse { name, response }),
            inline_data: None,
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
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
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
    ) -> Result<Vec<GeminiPart>> {
        //INFO: Build the request payload
        let request = GeminiRequest {
            contents: messages,
            system_instruction: system_instruction.map(|instruction| GeminiContent {
                role: None,
                parts: vec![GeminiPart::text(instruction.to_string())],
            }),
            tools,
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
        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini API response")?;

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

        Ok(first_candidate.content.parts.clone())
    }

    //INFO: Tests if the API key is valid by sending a simple request
    pub async fn test_connection(&self) -> Result<bool> {
        let request = vec![GeminiContent {
            role: Some("user".to_string()),
            parts: vec![GeminiPart::text("Say 'Hello' in one word.".to_string())],
        }];
        let result = self.send_chat(request, None, None).await;
        Ok(result.is_ok())
    }
}
