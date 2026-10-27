# MyNoteTaker

A small Windows meeting note-taker that runs on your machine. Start it when a meeting begins; it listens to your microphone and the remote participants (system audio), transcribes live, writes a markdown file, and adds an AI summary when you stop. Transcription is always local, and summaries are local by default; the summary can optionally go to an OpenAI-compatible endpoint you configure (see "Remote summary"). 

> **Built with AI.** This project was designed and written with the help of an AI coding assistant (Claude Code). It is a personal tool, provided as is. Review the code before relying on it.

## What it does

- Captures your mic and system audio (WASAPI loopback) at the same time.
- Transcribes live and on-device with [Foundry Local](https://www.foundrylocal.ai/) using the streaming model `nemotron-speech-streaming-en-0.6b` (Whisper is the fallback).
- Labels speakers `Me` / `Them` (dual mode), or `Meeting` when both streams are mixed into one session.
- Writes `YYYY-MM-DD_HHMM.md` into a folder you choose (default `Documents\MyNoteTaker`).
- Summarizes the transcript with a local chat model when you stop recording.
- Start/stop from the window or the global hotkey `Ctrl+Alt+N`.
- Targets both NVIDIA GPU machines and Intel CPU/NPU machines; the model and variant are configurable.

## Configuration

`%APPDATA%\MyNoteTaker\config.toml` (log: `%APPDATA%\MyNoteTaker\MyNoteTaker.log`). Options include `notes_dir`, `speech_model`, `chat_model`, `chat_variant`, `summarize`, `hotkey`, `capture_mode` (`dual` | `mixed`) and `loopback_device`. Models download from Foundry Local on first use (speech about 700 MB, chat 330 MB to 1 GB).

### Remote summary (optional)

By default the summary comes from a local Foundry model. Set `chat_provider = "openai"` (or pick "OpenAI-compatible endpoint" in the window) to use any OpenAI-compatible `/chat/completions` endpoint instead: Ollama, LM Studio, OpenAI, OpenRouter. Transcription is not affected. Keys:

- `chat_provider`: `"foundry"` (default) or `"openai"`.
- `chat_base_url`: default `http://localhost:11434/v1` (Ollama); LM Studio `http://localhost:1234/v1`; OpenAI `https://api.openai.com/v1`.
- `chat_api_model`: model name; required for `"openai"`.
- `chat_api_key_env`: name of an environment variable holding the API key (fallback; the key itself is never put in the config). Empty means no Authorization header.
- `chat_max_input_chars`: final-pass input budget (default 6000 for Foundry, 100000 for HTTP, so normal meetings are summarized in one pass).

The API key can be entered in the window instead. It is encrypted with Windows DPAPI for the current user on this machine and stored in `%APPDATA%\MyNoteTaker\chat_api_key.bin`; the file does not work if copied to another machine or user (enter the key again). A saved key takes precedence over `chat_api_key_env`.

With a non-local endpoint the transcript text is sent to that host, and the window says so next to the record button. If the call fails there is no fallback to the local model; the note says "summary unavailable".

## Building

This repository contains source only. No binaries are committed.

Requirements: Windows, the Rust toolchain (MSVC target) and network access, because the Foundry Local SDK fetches its native DLLs at build time.

```powershell
$env:FOUNDRY_LOCAL_RUNTIME_VERSION = '2.1.0'
cargo build --release -p mynotetaker
```

The executable is `target\release\MyNoteTaker.exe`. The native `*.dll` files from `target\release\build\...\out` must sit next to it. The release workflow does this automatically.


## Releases

Every push to `main` runs .github/workflows/release.yml. It builds on Windows and publishes a GitHub Release `v<version>.<run number>` with `MyNoteTaker-<tag>-windows-x64.zip` attached (exe, native DLLs, install scripts). The zip is never committed to the repo.

## Repository layout

- `app\` - the application (egui UI, audio capture, transcription, summarization).
- `installer\` - per-user install/uninstall scripts shipped in the release zip.
- `docs\FINDINGS.md` - notes from the early hardware and audio experiments.
