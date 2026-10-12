# MyNoteTaker

A small Windows meeting note-taker that runs entirely on your machine. Start it when a meeting begins; it listens to your microphone and the remote participants (system audio), transcribes live, writes a markdown file, and adds an AI summary when you stop. Nothing leaves the machine.

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

## Building

This repository contains source only. No binaries are committed.

Requirements: Windows, the Rust toolchain (MSVC target) and network access, because the Foundry Local SDK fetches its native DLLs at build time.

```powershell
$env:FOUNDRY_LOCAL_RUNTIME_VERSION = '2.1.0'
cargo build --release -p mynotetaker
```

The executable is `target\release\MyNoteTaker.exe`. The native `*.dll` files from `target\release\build\...\out` must sit next to it. The release workflow does this automatically.

The `sandbox\` scripts are the author's alternative cross-build setups (container and host-based `xwin`).

## Releases

Pushing a tag such as `v0.1.0` runs the GitHub Actions workflow in `.github/workflows/release.yml`. It builds on Windows and attaches `MyNoteTaker-<tag>-windows-x64.zip` to a GitHub Release. The zip contains the exe, the native DLLs and the install scripts, and is never committed to the repo.

## Repository layout

- `app\` - the application (egui UI, audio capture, transcription, summarization).
- `spikes\` - throwaway experiments (catalog check, loopback capture, dual sessions) and `FINDINGS.md`.
- `sandbox\` - build and packaging scripts.
- `note_taker_ai.md` - the original project brief.
