use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Gemini TTS voices supported by SlideVoice Studio. Mirrors the
/// reference Flutter and Swift apps exactly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Voice {
    #[default]
    Zephyr,
    Puck,
    Charon,
    Kore,
    Fenrir,
}

impl Voice {
    pub fn label(self) -> &'static str {
        match self {
            Voice::Zephyr => "Zephyr",
            Voice::Puck => "Puck",
            Voice::Charon => "Charon",
            Voice::Kore => "Kore",
            Voice::Fenrir => "Fenrir",
        }
    }
}

/// Slide entry transitions. Maps to FFmpeg `xfade` types in the encoder.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    None,
    Fade,
    #[default]
    Slide,
    Wipe,
    Zoom,
}

impl Transition {
    /// FFmpeg `xfade` transition name. `None` falls back to `fade` since
    /// the assembly path skips xfade entirely when every slide is `None`.
    pub fn xfade_name(self) -> &'static str {
        match self {
            Transition::None | Transition::Fade => "fade",
            Transition::Slide => "slideleft",
            Transition::Wipe => "wipeleft",
            Transition::Zoom => "zoomin",
        }
    }
}

impl FromStr for Transition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Transition::None),
            "fade" => Ok(Transition::Fade),
            "slide" => Ok(Transition::Slide),
            "wipe" => Ok(Transition::Wipe),
            "zoom" => Ok(Transition::Zoom),
            other => Err(format!("unknown transition: {other}")),
        }
    }
}
