// Gemini TTS integration for briefing audio generation
use crate::crypto::decrypt_token;
use crate::database::{queries, Database};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

const TTS_MODEL: &str = "gemini-2.5-flash-preview-tts";

#[derive(Debug, Serialize)]
struct TTSRequest {
    contents: Vec<TTSContent>,
    #[serde(rename = "generationConfig")]
    generation_config: TTSGenerationConfig,
}

#[derive(Debug, Serialize)]
struct TTSContent {
    parts: Vec<TTSPart>,
}

#[derive(Debug, Serialize)]
struct TTSPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct TTSGenerationConfig {
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<String>,
    #[serde(rename = "speechConfig")]
    speech_config: SpeechConfig,
}

#[derive(Debug, Serialize)]
struct SpeechConfig {
    #[serde(rename = "voiceConfig")]
    voice_config: VoiceConfig,
}

#[derive(Debug, Serialize)]
struct VoiceConfig {
    #[serde(rename = "prebuiltVoiceConfig")]
    prebuilt_voice_config: PrebuiltVoiceConfig,
}

#[derive(Debug, Serialize)]
struct PrebuiltVoiceConfig {
    #[serde(rename = "voiceName")]
    voice_name: String,
}

#[derive(Debug, Deserialize)]
struct TTSResponse {
    candidates: Option<Vec<TTSCandidate>>,
}

#[derive(Debug, Deserialize)]
struct TTSCandidate {
    content: TTSCandidateContent,
}

#[derive(Debug, Deserialize)]
struct TTSCandidateContent {
    parts: Vec<TTSResponsePart>,
}

#[derive(Debug, Deserialize)]
struct TTSResponsePart {
    #[serde(rename = "inlineData")]
    inline_data: Option<InlineData>,
}

#[derive(Debug, Deserialize)]
struct InlineData {
    #[allow(dead_code)]
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

/// Generate audio from text using Gemini TTS
pub async fn generate_audio(database: &Database, text: &str) -> Result<Vec<u8>> {
    let api_key = {
        let connection = database.connection.lock();
        let encrypted_key =
            queries::get_api_token(&connection, "gemini")?.context("Gemini API key not found")?;
        decrypt_token(&encrypted_key)?
    };

    // Clean text for speech (remove markdown)
    let link_regex = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    let clean_text = link_regex.replace_all(text, "$1").to_string();
    let clean_text = clean_text.replace(['*', '_', '#', '`'], "");

    let request = TTSRequest {
        contents: vec![TTSContent {
            parts: vec![TTSPart { text: clean_text }],
        }],
        generation_config: TTSGenerationConfig {
            response_modalities: vec!["AUDIO".to_string()],
            speech_config: SpeechConfig {
                voice_config: VoiceConfig {
                    prebuilt_voice_config: PrebuiltVoiceConfig {
                        // Kore is a soft, gentle female voice
                        voice_name: "Kore".to_string(),
                    },
                },
            },
        },
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        TTS_MODEL, api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send TTS request")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("TTS API error: {}", error_text);
    }

    let tts_response: TTSResponse = response
        .json()
        .await
        .context("Failed to parse TTS response")?;

    // Extract audio data from response
    let audio_data = tts_response
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content.parts.into_iter().next())
        .and_then(|p| p.inline_data)
        .context("No audio data in response")?;

    // Decode base64 audio
    let raw_audio = general_purpose::STANDARD
        .decode(&audio_data.data)
        .context("Failed to decode audio data")?;

    // Gemini 2.x returns raw PCM (16-bit, 24kHz, Mono).
    // We need to wrap it in a WAV header for the browser to play it.
    let wav_data = wrap_in_wav(raw_audio, 24000);

    Ok(wav_data)
}

fn wrap_in_wav(pcm_data: Vec<u8>, sample_rate: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(44 + pcm_data.len());
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size;

    // RIFF Header
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");

    // fmt subchunk
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    header.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 for PCM)
    header.extend_from_slice(&1u16.to_le_bytes()); // NumChannels (1 for Mono)
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // ByteRate (SampleRate * NumChannels * BitsPerSample/8)
    header.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign (NumChannels * BitsPerSample/8)
    header.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample

    // data subchunk
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_size.to_le_bytes());
    header.extend_from_slice(&pcm_data);

    header
}
