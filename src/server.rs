//! MCP server exposing the MiMo TTS tools over stdio.

use std::path::Path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::schemars::JsonSchema;
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::tts::{self, Synthesis};
use crate::voices::{PresetVoice, PRESET_VOICES};

/// Instructions advertised to MCP clients.
pub const SERVER_INSTRUCTIONS: &str =
    "Synthesize speech with the Xiaomi MiMo TTS service and save the audio to a file.";

/// Arguments accepted by the `synthesize_speech` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SynthesizeParams {
    /// Target text to read out loud.
    pub text: String,
    /// Preset voice id; run the `voices` command for the full list.
    pub voice: Option<String>,
    /// Optional style instruction, for example "(东北话)用轻松愉快的语气播报".
    pub style: Option<String>,
    /// Destination file or directory; the current directory is used when omitted.
    pub output_path: Option<String>,
}

/// Response payload of the `list_voices` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct VoicesResponse {
    /// Preset voices offered by the service.
    pub voices: Vec<PresetVoice>,
}

/// MCP server that exposes the MiMo TTS tools.
#[derive(Debug, Clone)]
pub struct MimoTtsServer {
    tool_router: ToolRouter<Self>,
}

impl MimoTtsServer {
    /// Create a server with every tool registered.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for MimoTtsServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl MimoTtsServer {
    /// List the preset voices available for speech synthesis.
    #[tool(description = "List the preset voices available for speech synthesis.")]
    pub async fn list_voices(&self) -> Result<Json<VoicesResponse>, String> {
        Ok(Json(VoicesResponse {
            voices: PRESET_VOICES.to_vec(),
        }))
    }

    /// Synthesize speech with MiMo TTS, write the audio file, and return its metadata.
    #[tool(description = "Synthesize speech with MiMo TTS, write the audio file, and return its metadata.")]
    pub async fn synthesize_speech(
        &self,
        Parameters(params): Parameters<SynthesizeParams>,
    ) -> Result<Json<Synthesis>, String> {
        let settings = config::load().map_err(tool_error)?;
        let output_path = params.output_path.as_deref().map(Path::new);

        let synthesis = tts::synthesize(
            &settings,
            &params.text,
            params.voice.as_deref(),
            params.style.as_deref(),
            output_path,
        )
        .await
        .map_err(tool_error)?;

        Ok(Json(synthesis))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MimoTtsServer {
    /// Describe the server and the capabilities it offers.
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}

/// Turn an error into a message that MCP clients receive as a tool failure.
fn tool_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Run the MCP server over stdio until the client disconnects.
pub async fn run() -> anyhow::Result<()> {
    let service = MimoTtsServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
