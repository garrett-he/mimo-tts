//! Command line interface of mimo-tts-mcp.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use mimo_tts_mcp::{config, server, tts, voices};

/// Command line arguments of the executable.
#[derive(Debug, Parser)]
#[command(
    name = "mimo-tts-mcp",
    version,
    about = "Generate speech audio with the Xiaomi MiMo TTS service."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Commands offered by the CLI.
#[derive(Debug, Subcommand)]
enum Command {
    /// Synthesize speech with MiMo TTS and write the audio to a file.
    Say {
        /// Target text to read out loud.
        text: String,
        /// Preset voice id; see the `voices` command.
        #[arg(long)]
        voice: Option<String>,
        /// Optional style instruction.
        #[arg(long)]
        style: Option<String>,
        /// Destination file or directory; defaults to the current directory.
        #[arg(long, default_value = ".")]
        output: PathBuf,
    },
    /// List the preset voices available for speech synthesis.
    Voices,
    /// Show the resolved configuration and the config file in use.
    Config,
    /// Run the MCP server over stdio for MCP clients.
    Serve,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch the parsed command.
async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Say {
            text,
            voice,
            style,
            output,
        } => {
            let settings = config::load()?;
            let synthesis = tts::synthesize(
                &settings,
                &text,
                voice.as_deref(),
                style.as_deref(),
                Some(output.as_path()),
            )
            .await?;
            println!("{}", synthesis.path.display());
            Ok(())
        }
        Command::Voices => {
            for voice in voices::PRESET_VOICES {
                println!("{}\t{}\t{}\t{}", voice.voice, voice.label, voice.language, voice.gender);
            }
            Ok(())
        }
        Command::Config => {
            let path = config::resolve_config_path();
            let state = if path.is_file() { "found" } else { "missing" };
            println!("config file: {} ({})", path.display(), state);

            let settings = config::load()?;
            println!("API key: configured ({})", settings.masked_api_key());
            println!("base url: {}", settings.base_url);
            println!("model: {}", settings.model);
            println!("voice: {}", settings.voice);
            println!("audio format: {}", settings.audio_format);
            Ok(())
        }
        Command::Serve => Ok(server::run().await?),
    }
}
