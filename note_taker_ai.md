# Project: Local meeting note-taker (Rust, Foundry Local, Windows)

## Context
I want a small personal Windows app that I start manually when a meeting begins.
It listens to the meeting (my mic plus the remote participants via system audio),
transcribes it live and on-device using Foundry Local, writes the transcript to a
markdown file in a folder I choose, and adds an AI summary when I stop it.
Nothing leaves the machine.

I have two target machines, and the app must work on both:
- **Machine A:** NVIDIA GPU
- **Machine B:** Intel CPU with NPU

Reference material (read these before writing code):
- Live transcription (Rust): https://learn.microsoft.com/en-us/azure/foundry-local/how-to/how-to-live-transcribe-audio?tabs=windows&pivots=programming-language-rust
- Voice-to-text tutorial (chat model summarization): https://learn.microsoft.com/en-us/azure/foundry-local/tutorials/tutorial-build-voice-to-text-note-taker?tabs=windows&pivots=programming-language-rust
- Samples: https://github.com/microsoft/foundry-local/tree/main/samples/rust
  (especially `live-audio-transcription`, `native-chat-completions`, `verify-winml`)
- Model catalog: https://www.foundrylocal.ai/models

Docs and SDK versions can drift. Check the actual `foundry-local-sdk` crate
API (docs.rs or the crate source) instead of trusting the docs snippets, and tell
me when they differ.

Known facts to account for:
- The streaming speech model `nemotron-speech-streaming-en-0.6b` only exists in
  the SDK 1.1.x+ catalog. Older SDKs report "model not found".
- Nemotron works through the live transcription session API, not the
  file-based `transcribe()` call. Whisper uses file-based transcription.
- The CLI (`foundry`) is versioned separately from the SDK and may show a
  different catalog. Trust the SDK catalog.

## Phase 0: Spikes (do these first, then STOP and report)
Build each one as a small throwaway binary in `spikes/`. Run every spike on
BOTH machines and record results per machine. For each, report what worked,
what didn't, and any numbers measured.

0. **Hardware and catalog check.** Run `verify-winml` and list the SDK catalog
   on each machine. Record which execution providers are registered and which
   variants (CPU / GPU / NPU) exist for the streaming speech model, Whisper,
   and candidate chat models. On Machine B, specifically check for NPU
   variants (OpenVINO via WinML) of the speech model. Confirm
   `nemotron-speech-streaming-en-0.6b` resolves.
1. **Loopback capture.** Capture system audio (WASAPI loopback on the default
   render device) and convert it to 16 kHz mono 16-bit PCM. Try cpal first
   (input stream on an output device), then the `wasapi` crate if needed.
   Done when: a WAV file of a YouTube clip playing through speakers sounds clean.
2. **Dual live sessions.** With one loaded streaming model, run two live
   transcription sessions at the same time: one fed by the mic, one fed by
   loopback. Done when: both produce final results concurrently, OR you've
   confirmed this isn't supported. In that case, prototype mixing both streams
   into one session.
   Fallback: if the streaming model isn't usable on a machine, implement
   Whisper with voice-activity-based WAV segments and file transcription, and
   report the latency difference.
3. **Bluetooth device switching.** With a Bluetooth headset, check what happens
   to the loopback stream when a Teams or Zoom call opens the headset mic
   (Windows may switch to a "Hands-Free" endpoint), and when the headset
   disconnects mid-capture. Prototype reopening the stream on device-change
   notifications (IMMNotificationClient via the `windows` crate). Also check
   default device vs. default *communications* device behavior, and whether
   the Bluetooth hands-free mic delivers narrowband (8 kHz) audio.
4. **Summary model and device placement.** Feed a ~1-hour sample transcript to
   2–3 chat models from the catalog (e.g. phi-4-mini, a qwen2.5 1.5B–7B
   variant). Report action-item quality and summarization time. Then compare:
   (a) swap approach: unload speech, load chat, summarize, reload speech;
   (b) keep both loaded on different devices.
       Machine A: speech on CPU, chat on GPU.
       Machine B: speech on NPU with chat on CPU/iGPU if a speech NPU variant
       exists, otherwise speech on CPU with chat on NPU.
   Confirm whether the SDK lets me select a variant/device per alias.
5. **Startup and resource cost.** Measure first-run and warm-start time for
   execution-provider registration and model load, plus idle memory of the
   loaded speech model. On Machine B, compare CPU usage and battery drain
   during a 30-minute transcription on NPU vs. CPU.

Write findings to `spikes/FINDINGS.md` with a section per machine, then stop
and wait for my go-ahead.

## Phase 1: MVP (only after I approve the Phase 0 findings)

### Behavior
- Tray app, launched manually. Load the speech model at launch (and the chat
  model too, if Spike 4 shows that keeping both loaded works) so starting a
  meeting is instant. No console window (`#![windows_subsystem = "windows"]`);
  log to a file.
- Global hotkey (configurable, default `Ctrl+Alt+N`) and tray menu item toggle
  recording. The tray icon shows the recording state.
- While recording: mic → session labeled **Me**; loopback → session labeled
  **Them** (or one mixed session, per Spike 2; make it a config option).
- Only final results are written. Partials may show in the tray tooltip.
- On stop: close the sessions, then generate the summary (swap or resident
  chat model, per Spike 4) and prepend it to the note.
- Handle device changes per Spike 3 without losing the session.

### Output format
One file per meeting in the configured folder, e.g. `2026-10-11_1430.md`:
```
---
date: 2026-10-11T14:30:00
duration: 00:47:12
title: (optional, editable from tray)
---
## Summary
## Decisions
## Action items
## Open questions

## Transcript
- [14:32:10] **Me:** ...
- [14:32:18] **Them:** ...
```
Append transcript lines as they arrive and flush after each line, so a crash
never loses what was already said. Keep the raw transcript below the summary.

### Config (`config.toml` in `%APPDATA%\meeting-notes\`)
notes_dir, speech_model alias, speech_variant (optional override),
language, chat_model alias, chat_variant (optional override),
summarize (bool), keep_chat_loaded (bool), hotkey,
capture_mode (dual | mixed), loopback_device (default | communications).
By default, aliases pick the variant automatically. The overrides let me pin a
device per machine.

### Suggested module layout
`main.rs` (tray, hotkey, event loop; tokio on its own runtime thread),
`audio/` (mic, loopback, convert/resample, device watcher), `transcriber.rs`
(model lifecycle, sessions, Whisper fallback), `notes.rs` (file naming,
frontmatter, append), `summarize.rs`, `config.rs`.

### Non-goals (for now)
Autostart at login, identifying individual remote speakers, macOS and Linux,
cloud anything, a GUI beyond the tray.

## Working rules
- Target Windows 11. Use the `winml` feature of the SDK.
- Pin `foundry-local-sdk` to 1.1.x or later.
- Small commits per step; keep `cargo clippy` clean.
- Ask me before adding heavyweight dependencies or changing the output format.
- If something in this plan turns out to be wrong or impossible, say so and
  propose an alternative instead of working around it silently.