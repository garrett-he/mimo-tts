//! Speech synthesis through the MiMo TTS API.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use reqwest::Client;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;

use crate::config::Settings;
use crate::voices::{is_preset_voice, preset_voice_ids};

/// Request timeout applied to MiMo TTS calls.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Errors raised by the MiMo TTS client.
#[derive(Debug, Error)]
pub enum TtsError {
    /// The target text is blank.
    #[error("Target text for speech synthesis must not be empty.")]
    EmptyText,
    /// The requested voice is not part of the preset catalogue.
    #[error("Unknown preset voice \"{voice}\"; available voices: {available}")]
    UnknownVoice {
        /// Voice that was requested.
        voice: String,
        /// Voices the client accepts.
        available: String,
    },
    /// The HTTP request could not be completed.
    #[error("MiMo TTS request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// The service answered with an error status.
    #[error("MiMo TTS returned HTTP {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body, truncated for readability.
        body: String,
    },
    /// The response body is not valid JSON.
    #[error("MiMo TTS response is not valid JSON: {0}")]
    Malformed(String),
    /// The response does not contain any choice.
    #[error("MiMo TTS response contains no choices.")]
    MissingChoices,
    /// The response does not contain an audio payload.
    #[error("MiMo TTS response contains no audio data.")]
    MissingAudio,
    /// The audio payload is not valid base64.
    #[error("MiMo TTS returned an undecodable audio payload: {0}")]
    Decode(String),
    /// The audio file could not be written.
    #[error("Cannot write audio file {}: {source}", path.display())]
    Write {
        /// Destination that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
}

/// Result of a synthesis call.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Synthesis {
    /// Absolute path of the written audio file.
    pub path: PathBuf,
    /// Model used for the request.
    pub model: String,
    /// Voice used for the request.
    pub voice: String,
    /// Audio format of the written file.
    pub audio_format: String,
    /// Size of the written file in bytes.
    pub size_bytes: u64,
}

/// Create an HTTP client with the configured request timeout.
pub fn build_client() -> Result<Client, TtsError> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(TtsError::Request)
}

/// Build the OpenAI-compatible request body for a synthesis call.
pub fn build_request_body(text: &str, style: Option<&str>, voice: &str, settings: &Settings) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(style) = style.map(str::trim).filter(|value| !value.is_empty()) {
        messages.push(json!({ "role": "user", "content": style }));
    }
    messages.push(json!({ "role": "assistant", "content": text.trim() }));

    json!({
        "model": settings.model,
        "messages": messages,
        "audio": { "format": settings.audio_format, "voice": voice },
    })
}

/// Decode the base64 audio payload of a chat completion.
pub fn decode_audio(response: &Value) -> Result<Vec<u8>, TtsError> {
    let data = response
        .pointer("/choices/0/message/audio/data")
        .and_then(Value::as_str)
        .ok_or(if response.get("choices").is_none() {
            TtsError::MissingChoices
        } else {
            TtsError::MissingAudio
        })?;

    STANDARD
        .decode(data)
        .map_err(|error| TtsError::Decode(error.to_string()))
}

/// Resolve the destination path of the synthesized audio.
///
/// Relative paths are resolved against the current directory, which is also used when no
/// path is given. A path pointing at a directory receives a timestamped file name, while a
/// file path is used as it is.
pub fn resolve_output_path(output_path: Option<&Path>, settings: &Settings) -> PathBuf {
    let directory = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = match output_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => directory.join(path),
        None => directory,
    };
    let candidate = normalize(&candidate);

    if candidate.is_dir() {
        candidate.join(format!("mimo-tts-{}{}", timestamp_millis(), settings.audio_suffix()))
    } else {
        candidate
    }
}

/// Drop `.` components so that resolved paths stay readable.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::CurDir) {
            normalized.push(component.as_os_str());
        }
    }
    normalized
}

/// Resolve the voice to use, validating it against the preset catalogue.
pub fn resolve_voice(voice: Option<&str>, settings: &Settings) -> Result<String, TtsError> {
    let voice = voice
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| settings.voice.trim());

    if !is_preset_voice(voice) {
        return Err(TtsError::UnknownVoice {
            voice: voice.to_string(),
            available: preset_voice_ids().join(", "),
        });
    }
    Ok(voice.to_string())
}

/// Synthesize `text` into an audio file and describe the written file.
pub async fn synthesize(
    settings: &Settings,
    text: &str,
    voice: Option<&str>,
    style: Option<&str>,
    output_path: Option<&Path>,
) -> Result<Synthesis, TtsError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(TtsError::EmptyText);
    }
    let voice = resolve_voice(voice, settings)?;

    let url = format!("{}/chat/completions", settings.base_url.trim_end_matches('/'));
    let body = build_request_body(text, style, &voice, settings);
    let response = build_client()?
        .post(url)
        .bearer_auth(&settings.api_key)
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let payload = response.text().await?;
    if !status.is_success() {
        return Err(TtsError::Api {
            status: status.as_u16(),
            body: truncate(&payload, 500),
        });
    }

    let parsed: Value = serde_json::from_str(&payload).map_err(|error| TtsError::Malformed(error.to_string()))?;
    let audio = decode_audio(&parsed)?;

    let destination = resolve_output_path(output_path, settings);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| TtsError::Write {
            path: destination.clone(),
            source,
        })?;
    }
    fs::write(&destination, &audio).map_err(|source| TtsError::Write {
        path: destination.clone(),
        source,
    })?;

    Ok(Synthesis {
        path: destination,
        model: settings.model.clone(),
        voice,
        audio_format: settings.audio_format.clone(),
        size_bytes: audio.len() as u64,
    })
}

/// Return the current Unix time in milliseconds.
fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default()
}

/// Shorten a response body so that error messages stay readable.
fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let shortened: String = value.chars().take(limit).collect();
    format!("{shortened}…")
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::STANDARD;
    use serde_json::json;

    use super::*;
    use crate::config::{DEFAULT_AUDIO_FORMAT, DEFAULT_BASE_URL, DEFAULT_MODEL};

    fn settings() -> Settings {
        Settings {
            api_key: "test-key".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            voice: "mimo_default".to_string(),
            audio_format: DEFAULT_AUDIO_FORMAT.to_string(),
        }
    }

    #[test]
    fn request_body_follows_the_documented_shape() {
        let body = build_request_body("你好", Some("温柔一点"), "Chloe", &settings());

        assert_eq!(body["model"], json!(DEFAULT_MODEL));
        assert_eq!(body["messages"][0], json!({ "role": "user", "content": "温柔一点" }));
        assert_eq!(body["messages"][1], json!({ "role": "assistant", "content": "你好" }));
        assert_eq!(body["audio"], json!({ "format": "wav", "voice": "Chloe" }));
    }

    #[test]
    fn request_body_without_style_only_carries_the_target_text() {
        let body = build_request_body("你好", None, "mimo_default", &settings());

        assert_eq!(body["messages"].as_array().map(Vec::len), Some(1));
        assert_eq!(body["messages"][0], json!({ "role": "assistant", "content": "你好" }));
    }

    #[test]
    fn audio_payload_is_decoded() {
        let payload = STANDARD.encode(b"riff-bytes");
        let response = json!({ "choices": [{ "message": { "audio": { "data": payload } } }] });

        assert_eq!(decode_audio(&response).expect("audio"), b"riff-bytes");
    }

    #[test]
    fn response_without_choices_is_rejected() {
        let response = json!({ "id": "chatcmpl" });

        assert!(matches!(
            decode_audio(&response).expect_err("no choices"),
            TtsError::MissingChoices
        ));
    }

    #[test]
    fn response_without_audio_is_rejected() {
        let response = json!({ "choices": [{ "message": { "content": "" } }] });

        assert!(matches!(
            decode_audio(&response).expect_err("no audio"),
            TtsError::MissingAudio
        ));
    }

    #[test]
    fn undecodable_payload_is_rejected() {
        let response = json!({ "choices": [{ "message": { "audio": { "data": "not-base64!!" } } }] });

        assert!(matches!(
            decode_audio(&response).expect_err("bad base64"),
            TtsError::Decode(_)
        ));
    }

    #[test]
    fn empty_text_is_rejected() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let error = runtime
            .block_on(synthesize(&settings(), "   ", None, None, None))
            .expect_err("blank text");

        assert!(matches!(error, TtsError::EmptyText));
    }

    #[test]
    fn unknown_voice_is_rejected() {
        assert!(matches!(
            resolve_voice(Some("not-a-voice"), &settings()).expect_err("unknown voice"),
            TtsError::UnknownVoice { .. }
        ));
    }

    #[test]
    fn configured_voice_is_used_when_none_is_given() {
        assert_eq!(resolve_voice(None, &settings()).expect("voice"), "mimo_default");
    }

    #[test]
    fn default_output_path_uses_the_current_directory() {
        let path = resolve_output_path(None, &settings());

        assert_eq!(path.parent(), env::current_dir().ok().as_deref());
        assert_eq!(path.extension().and_then(|value| value.to_str()), Some("wav"));
    }

    #[test]
    fn output_directory_receives_a_generated_file_name() {
        let directory = env::temp_dir();
        let path = resolve_output_path(Some(directory.as_path()), &settings());

        assert_eq!(path.parent(), Some(directory.as_path()));
        assert!(path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("mimo-tts-")));
    }

    #[test]
    fn explicit_output_path_is_kept() {
        let path = env::temp_dir().join("custom.wav");

        assert_eq!(resolve_output_path(Some(path.as_path()), &settings()), path);
    }

    #[test]
    fn relative_output_path_is_resolved_against_the_current_directory() {
        let path = resolve_output_path(Some(Path::new("custom.wav")), &settings());

        assert!(path.is_absolute());
        assert_eq!(path.file_name().and_then(|value| value.to_str()), Some("custom.wav"));
    }
}
