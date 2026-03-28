//! Configuration loading for the MiMo TTS client and the MCP server.

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::voices::DEFAULT_VOICE;

/// Directory inside the home directory that holds the configuration file.
pub const APP_DIR: &str = ".mimo-tts-mcp";
/// Name of the JSON configuration file.
pub const CONFIG_FILE: &str = "config.json";

/// Environment variables; they take precedence over the configuration file.
pub const ENV_API_KEY: &str = "MIMO_API_KEY";
pub const ENV_CONFIG_PATH: &str = "MIMO_TTS_MCP_CONFIG";
pub const ENV_BASE_URL: &str = "MIMO_TTS_MCP_BASE_URL";
pub const ENV_MODEL: &str = "MIMO_TTS_MCP_MODEL";
pub const ENV_VOICE: &str = "MIMO_TTS_MCP_VOICE";
pub const ENV_AUDIO_FORMAT: &str = "MIMO_TTS_MCP_AUDIO_FORMAT";

/// Base URL of the OpenAI-compatible MiMo endpoint.
pub const DEFAULT_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
/// Model used when none is configured.
pub const DEFAULT_MODEL: &str = "mimo-v2.5-tts";
/// Audio format used when none is configured.
pub const DEFAULT_AUDIO_FORMAT: &str = "wav";
/// Audio formats supported by the client.
pub const SUPPORTED_AUDIO_FORMATS: [&str; 2] = ["wav", "pcm16"];

/// Errors raised while resolving the configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No API key was configured.
    #[error(
        "MiMo API key is missing: set the {} environment variable or the \"api_key\" entry in {}",
        ENV_API_KEY,
        path.display()
    )]
    MissingApiKey {
        /// Configuration file that was inspected.
        path: PathBuf,
    },
    /// The configured audio format is not supported.
    #[error("Unsupported audio format \"{value}\": expected one of {supported}")]
    UnsupportedAudioFormat {
        /// Format found in the configuration.
        value: String,
        /// Formats the client accepts.
        supported: String,
    },
    /// The configuration file could not be read.
    #[error("Cannot read config file {}: {source}", path.display())]
    ReadFile {
        /// Configuration file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The configuration file is not valid JSON.
    #[error("Cannot parse config file {}: {source}", path.display())]
    ParseFile {
        /// Configuration file that could not be parsed.
        path: PathBuf,
        /// Underlying JSON error.
        source: serde_json::Error,
    },
}

/// Resolved settings used to call the MiMo TTS API.
#[derive(Clone)]
pub struct Settings {
    /// API key; never printed in full.
    pub api_key: String,
    /// Base URL of the OpenAI-compatible API.
    pub base_url: String,
    /// Model identifier.
    pub model: String,
    /// Default preset voice.
    pub voice: String,
    /// Audio format, either `wav` or `pcm16`.
    pub audio_format: String,
}

impl Settings {
    /// Return the file suffix matching the configured audio format.
    pub fn audio_suffix(&self) -> &'static str {
        if self.audio_format == "pcm16" {
            ".pcm"
        } else {
            ".wav"
        }
    }

    /// Return the API key with all but the first four characters masked.
    pub fn masked_api_key(&self) -> String {
        if self.api_key.chars().count() > 4 {
            let prefix: String = self.api_key.chars().take(4).collect();
            format!("{prefix}***")
        } else {
            "***".to_string()
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("api_key", &self.masked_api_key())
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("voice", &self.voice)
            .field("audio_format", &self.audio_format)
            .finish()
    }
}

/// Optional values read from the JSON configuration file.
#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    /// MiMo API key.
    pub api_key: Option<String>,
    /// Base URL of the OpenAI-compatible API.
    pub base_url: Option<String>,
    /// Model identifier.
    pub model: Option<String>,
    /// Default preset voice.
    pub voice: Option<String>,
    /// Audio format, either `wav` or `pcm16`.
    pub audio_format: Option<String>,
}

/// Return the configuration file path used when the override is not set.
pub fn default_config_path() -> PathBuf {
    match dirs::home_dir() {
        Some(home) => home.join(APP_DIR).join(CONFIG_FILE),
        None => PathBuf::from(APP_DIR).join(CONFIG_FILE),
    }
}

/// Resolve the configuration file path, honouring the `MIMO_TTS_MCP_CONFIG` override.
pub fn resolve_config_path() -> PathBuf {
    match non_blank(env::var(ENV_CONFIG_PATH).ok()) {
        Some(value) => PathBuf::from(value),
        None => default_config_path(),
    }
}

/// Read the JSON configuration file, returning empty values when it does not exist.
pub fn read_config_file(path: &Path) -> Result<ConfigFile, ConfigError> {
    if !path.is_file() {
        return Ok(ConfigFile::default());
    }
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| ConfigError::ParseFile {
        path: path.to_path_buf(),
        source,
    })
}

/// Load the settings from the process environment and the configuration file.
pub fn load() -> Result<Settings, ConfigError> {
    let path = resolve_config_path();
    load_with(|name| env::var(name).ok(), &path)
}

/// Load the settings from `env` and `config_path`; environment values win.
pub fn load_with<F>(env: F, config_path: &Path) -> Result<Settings, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let file = read_config_file(config_path)?;

    let pick = |env_name: &str, file_value: Option<&String>, fallback: &str| -> String {
        non_blank(env(env_name))
            .or_else(|| non_blank(file_value.cloned()))
            .unwrap_or_else(|| fallback.to_string())
    };

    let api_key = non_blank(env(ENV_API_KEY)).or_else(|| non_blank(file.api_key.clone()));
    let Some(api_key) = api_key else {
        return Err(ConfigError::MissingApiKey {
            path: config_path.to_path_buf(),
        });
    };

    let audio_format = pick(ENV_AUDIO_FORMAT, file.audio_format.as_ref(), DEFAULT_AUDIO_FORMAT);
    if !SUPPORTED_AUDIO_FORMATS.contains(&audio_format.as_str()) {
        return Err(ConfigError::UnsupportedAudioFormat {
            value: audio_format,
            supported: SUPPORTED_AUDIO_FORMATS.join(", "),
        });
    }

    Ok(Settings {
        api_key,
        base_url: pick(ENV_BASE_URL, file.base_url.as_ref(), DEFAULT_BASE_URL),
        model: pick(ENV_MODEL, file.model.as_ref(), DEFAULT_MODEL),
        voice: pick(ENV_VOICE, file.voice.as_ref(), DEFAULT_VOICE),
        audio_format,
    })
}

/// Return the trimmed value when it holds non-blank text.
fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write_config(directory: &TempDir, contents: &str) -> PathBuf {
        let path = directory.path().join("config.json");
        fs::write(&path, contents).expect("config file");
        path
    }

    #[test]
    fn defaults_are_applied() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(&directory, r#"{"api_key": "key"}"#);

        let settings = load_with(|_| None, &path).expect("settings");

        assert_eq!(settings.base_url, DEFAULT_BASE_URL);
        assert_eq!(settings.model, DEFAULT_MODEL);
        assert_eq!(settings.voice, DEFAULT_VOICE);
        assert_eq!(settings.audio_format, "wav");
        assert_eq!(settings.audio_suffix(), ".wav");
    }

    #[test]
    fn environment_wins_over_the_config_file() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(
            &directory,
            r#"{"api_key": "file-key", "base_url": "https://file.example/v1"}"#,
        );

        let settings = load_with(|name| (name == ENV_API_KEY).then(|| "env-key".to_string()), &path).expect("settings");

        assert_eq!(settings.api_key, "env-key");
        assert_eq!(settings.base_url, "https://file.example/v1");
    }

    #[test]
    fn config_file_is_used_when_the_environment_is_silent() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(
            &directory,
            r#"{"api_key": "file-key", "model": "mimo-v2.5-tts", "voice": "Chloe"}"#,
        );

        let settings = load_with(|_| None, &path).expect("settings");

        assert_eq!(settings.api_key, "file-key");
        assert_eq!(settings.voice, "Chloe");
    }

    #[test]
    fn output_directory_entries_are_ignored() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(&directory, r#"{"api_key": "key", "output_dir": "/somewhere"}"#);

        let settings = load_with(|_| None, &path).expect("settings");

        assert_eq!(settings.voice, DEFAULT_VOICE);
    }

    #[test]
    fn missing_api_key_is_reported() {
        let directory = TempDir::new().expect("temp dir");
        let path = directory.path().join("missing.json");

        let error = load_with(|_| None, &path).expect_err("missing key");

        assert!(error.to_string().contains(ENV_API_KEY));
        assert!(error.to_string().contains("missing.json"));
    }

    #[test]
    fn unsupported_audio_format_is_reported() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(&directory, r#"{"api_key": "key", "audio_format": "mp3"}"#);

        let error = load_with(|_| None, &path).expect_err("bad format");

        assert!(error.to_string().contains("Unsupported audio format"));
    }

    #[test]
    fn pcm16_uses_the_pcm_suffix() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(&directory, r#"{"api_key": "key", "audio_format": "pcm16"}"#);

        let settings = load_with(|_| None, &path).expect("settings");

        assert_eq!(settings.audio_suffix(), ".pcm");
    }

    #[test]
    fn invalid_json_is_reported() {
        let directory = TempDir::new().expect("temp dir");
        let path = write_config(&directory, "{");

        let error = load_with(|_| None, &path).expect_err("bad json");

        assert!(error.to_string().contains("Cannot parse config file"));
    }

    #[test]
    fn missing_config_file_is_not_an_error() {
        let directory = TempDir::new().expect("temp dir");
        let path = directory.path().join("absent.json");

        let file = read_config_file(&path).expect("empty config");

        assert!(file.api_key.is_none());
    }

    #[test]
    fn default_config_path_lives_in_the_app_directory() {
        let path = default_config_path();

        assert!(path.ends_with(Path::new(APP_DIR).join(CONFIG_FILE)));
    }

    #[test]
    fn debug_output_masks_the_api_key() {
        let settings = Settings {
            api_key: "super-secret-value".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            voice: DEFAULT_VOICE.to_string(),
            audio_format: DEFAULT_AUDIO_FORMAT.to_string(),
        };

        let rendered = format!("{settings:?}");

        assert!(!rendered.contains("super-secret-value"));
        assert!(rendered.contains("supe***"));
    }
}
