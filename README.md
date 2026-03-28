# mimo-tts-mcp

[![license](https://img.shields.io/badge/license-MIT-blue)](https://github.com/garrett-he/mimo-tts-mcp/blob/main/LICENSE)
[![ci](https://github.com/garrett-he/mimo-tts-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/garrett-he/mimo-tts-mcp/actions/workflows/ci.yml)

MCP server that generates audio via the Xiaomi MiMo TTS service.

## Requirements

- Rust 1.85 or later
- A MiMo API key, see the [first API call guide](https://mimo.mi.com/docs/zh-CN/quick-start/summary/first-api-call)

## Configuration

Settings come from environment variables first and fall back to the JSON file at
`~/.mimo-tts-mcp/config.json` (override its location with `MIMO_TTS_MCP_CONFIG`):

```json
{
  "api_key": "replace-with-your-api-key",
  "base_url": "https://api.xiaomimimo.com/v1",
  "model": "mimo-v2.5-tts",
  "voice": "mimo_default",
  "audio_format": "wav"
}
```

| Variable | Purpose |
| --- | --- |
| `MIMO_API_KEY` | API key; takes precedence over the config file |
| `MIMO_TTS_MCP_CONFIG` | Path of the config file |
| `MIMO_TTS_MCP_BASE_URL` | API base URL, defaults to `https://api.xiaomimimo.com/v1` |
| `MIMO_TTS_MCP_MODEL` | Model id, defaults to `mimo-v2.5-tts` |
| `MIMO_TTS_MCP_VOICE` | Default preset voice id, defaults to `mimo_default` |
| `MIMO_TTS_MCP_AUDIO_FORMAT` | Audio format, either `wav` or `pcm16` |

Generated audio is written to the current directory unless the CLI receives an
`--output` path; a path pointing at a directory receives a timestamped file name.

## Command line

```bash
cargo run --release -- voices
cargo run --release -- config
cargo run --release -- say "你好，世界" --voice Chloe --style "(东北话)用轻松的语气"
```

## MCP server

The server speaks MCP over stdio. Register the built binary with an MCP client:

```json
{
  "mcpServers": {
    "mimo-tts-mcp": {
      "command": "path/to/mimo-tts-mcp",
      "args": ["serve"]
    }
  }
}
```

Two tools are exposed: `list_voices` returns the preset voice catalogue, while
`synthesize_speech` reads the text out loud, writes the audio file, and returns
its path along with the model, voice, format and size.

## License

Copyright (C) 2026 Garrett HE <garrett.he@outlook.com>

MIT License, see [LICENSE](./LICENSE).
