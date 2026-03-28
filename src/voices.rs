//! Preset voices offered by the MiMo TTS service.

use schemars::JsonSchema;
use serde::Serialize;

/// Identifier of the voice used when none is configured.
pub const DEFAULT_VOICE: &str = "mimo_default";

/// A preset voice supported by the `mimo-v2.5-tts` model.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub struct PresetVoice {
    /// Voice identifier sent to the API.
    pub voice: &'static str,
    /// Human readable label.
    pub label: &'static str,
    /// Language code of the voice.
    pub language: &'static str,
    /// Gender label of the voice.
    pub gender: &'static str,
}

/// Preset voices supported by the `mimo-v2.5-tts` model.
pub const PRESET_VOICES: [PresetVoice; 9] = [
    PresetVoice {
        voice: "mimo_default",
        label: "MiMo-Default",
        language: "auto",
        gender: "auto",
    },
    PresetVoice {
        voice: "冰糖",
        label: "冰糖",
        language: "zh",
        gender: "female",
    },
    PresetVoice {
        voice: "茉莉",
        label: "茉莉",
        language: "zh",
        gender: "female",
    },
    PresetVoice {
        voice: "苏打",
        label: "苏打",
        language: "zh",
        gender: "male",
    },
    PresetVoice {
        voice: "白桦",
        label: "白桦",
        language: "zh",
        gender: "male",
    },
    PresetVoice {
        voice: "Mia",
        label: "Mia",
        language: "en",
        gender: "female",
    },
    PresetVoice {
        voice: "Chloe",
        label: "Chloe",
        language: "en",
        gender: "female",
    },
    PresetVoice {
        voice: "Milo",
        label: "Milo",
        language: "en",
        gender: "male",
    },
    PresetVoice {
        voice: "Dean",
        label: "Dean",
        language: "en",
        gender: "male",
    },
];

/// Return the identifiers of all preset voices.
pub fn preset_voice_ids() -> Vec<&'static str> {
    PRESET_VOICES.iter().map(|voice| voice.voice).collect()
}

/// Return whether `voice` matches a preset voice.
pub fn is_preset_voice(voice: &str) -> bool {
    PRESET_VOICES.iter().any(|preset| preset.voice == voice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_voice_is_preset() {
        assert!(is_preset_voice(DEFAULT_VOICE));
    }

    #[test]
    fn identifiers_are_unique() {
        let mut ids = preset_voice_ids();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);
        assert!(ids.contains(&"Chloe"));
    }

    #[test]
    fn unknown_voice_is_rejected() {
        assert!(!is_preset_voice("not-a-voice"));
    }
}
