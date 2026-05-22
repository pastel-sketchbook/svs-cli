//! Externalized prompts and model names loaded from `prompts.ron`.

use serde::Deserialize;
use std::sync::LazyLock;

static CONFIG_STR: &str = include_str!("../prompts.ron");

/// Prompt and model configuration parsed from `prompts.ron`.
#[derive(Debug, Deserialize)]
pub struct PromptsConfig {
    pub notes_model: String,
    pub tts_model: String,
    pub notes_prompt: String,
    pub greeting_first: String,
    pub greeting_rest: String,
    pub tts_prompt: String,
}

pub static PROMPTS: LazyLock<PromptsConfig> = LazyLock::new(|| {
    // RON is embedded at compile time; parse failure indicates a build-time bug.
    ron::from_str(CONFIG_STR).expect("failed to parse prompts.ron — check syntax")
});
