//! Thin async client for the two Gemini endpoints SlideVoice needs:
//! speaker-note extraction (vision) and text-to-speech.

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::PROMPTS;
use crate::models::Voice;

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub fn default_notes_model() -> &'static str {
    &PROMPTS.notes_model
}

pub fn tts_model() -> &'static str {
    &PROMPTS.tts_model
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    http: reqwest::Client,
    api_key: String,
}

impl GeminiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("svs-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_mins(2))
            .build()
            .context("building reqwest client")?;
        Ok(Self {
            http,
            api_key: api_key.into(),
        })
    }

    /// Ask Gemini to write a polished presenter script for the given slide
    /// image. The prompt mirrors `gemini_service.dart` / `GeminiService.swift`.
    pub async fn extract_notes(
        &self,
        image_jpeg: &[u8],
        is_first_slide: bool,
        notes_model: &str,
    ) -> Result<String> {
        let greeting_rule = if is_first_slide {
            &PROMPTS.greeting_first
        } else {
            &PROMPTS.greeting_rest
        };

        let prompt = PROMPTS
            .notes_prompt
            .replace("{greeting_rule}", greeting_rule);

        let body = json!({
            "contents": [{
                "parts": [
                    {"inline_data": {"mime_type": "image/jpeg", "data": B64.encode(image_jpeg)}},
                    {"text": prompt},
                ]
            }]
        });

        let url = format!("{GEMINI_BASE_URL}/models/{notes_model}:generateContent");
        let resp = self
            .http
            .post(&url)
            .query(&[("key", &self.api_key)])
            .json(&body)
            .send()
            .await
            .context("POST :generateContent (notes)")?;

        let resp = ensure_ok(resp).await?;
        let parsed: GenerateContentResponse =
            resp.json().await.context("parsing notes response")?;
        first_text(&parsed).ok_or_else(|| anyhow!("Gemini returned no text candidates"))
    }

    /// Generate TTS audio for `text` in the chosen `voice`. Returns raw PCM
    /// bytes (signed 16-bit LE, mono, 24 kHz) — Gemini TTS preview's native
    /// output format.
    pub async fn generate_speech(&self, text: &str, voice: Voice) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            bail!("cannot synthesize empty text");
        }

        let tts_text = PROMPTS.tts_prompt.replace("{text}", text);

        let body = json!({
            "contents": [{
                "parts": [{"text": tts_text}],
            }],
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": {"voiceName": voice.label()},
                    }
                }
            }
        });

        let tts_model = tts_model();
        let url = format!("{GEMINI_BASE_URL}/models/{tts_model}:generateContent");
        let resp = self
            .http
            .post(&url)
            .query(&[("key", &self.api_key)])
            .json(&body)
            .send()
            .await
            .context("POST :generateContent (tts)")?;

        let resp = ensure_ok(resp).await?;
        let parsed: GenerateContentResponse = resp.json().await.context("parsing tts response")?;
        let b64 = first_inline_data(&parsed)
            .ok_or_else(|| anyhow!("Gemini returned no audio candidates"))?;
        B64.decode(b64).context("decoding base64 audio")
    }
}

async fn ensure_ok(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    bail!("Gemini request failed: HTTP {status}: {body}")
}

#[derive(Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Value>,
}

fn first_text(resp: &GenerateContentResponse) -> Option<String> {
    let parts = resp
        .candidates
        .first()
        .and_then(|c| c.content.as_ref())
        .map(|c| &c.parts)?;
    for part in parts {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

fn first_inline_data(resp: &GenerateContentResponse) -> Option<&str> {
    let parts = resp
        .candidates
        .first()
        .and_then(|c| c.content.as_ref())
        .map(|c| &c.parts)?;
    for part in parts {
        let inline = part.get("inlineData").or_else(|| part.get("inline_data"))?;
        if let Some(data) = inline.get("data").and_then(|d| d.as_str())
            && !data.is_empty()
        {
            return Some(data);
        }
    }
    None
}
