//! Recording lifecycle, independent of the window: load models, start/stop a meeting, write the
//! note, summarize, switch models. The window talks to it over a command channel and reads the
//! shared state in [`Ui`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::Local;
use foundry_local_sdk::FoundryLocalManager;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::task::JoinHandle;

use crate::audio::{self, Capture, Source};
use crate::config::{self, CaptureMode, Config, LoopbackDevice};
use crate::notes::{Line, Note};
use crate::summarize::Summarizer;
use crate::transcriber::{self, PartialSink, Progress, Transcriber};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiState {
    #[default]
    Loading,
    Idle,
    Recording,
    Summarizing,
    Error,
    /// Backend finished (after a quit request); the window may close.
    Exited,
}

pub enum Cmd {
    Toggle,
    /// Start/stop dictating into the meeting context (idle only).
    Dictate,
    Quit,
    ApplyModels { speech: String, chat: String },
    SetNotesDir(PathBuf),
}

/// A catalog model offered in a dropdown.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub alias: String,
    pub size_mb: Option<u64>,
    /// Already downloaded; otherwise selecting it downloads it from Foundry Local.
    pub cached: bool,
}

#[derive(Default)]
pub struct Shared {
    pub state: UiState,
    pub status: String,
    pub partial: String,
    /// Most recent finished transcript lines (newest last).
    pub recent: Vec<String>,
    pub speech_models: Vec<ModelEntry>,
    pub chat_models: Vec<ModelEntry>,
    pub last_note: Option<PathBuf>,
    /// Why this meeting is happening; typed or dictated, fed to the summarizer, cleared after save.
    pub context: String,
    pub dictating: bool,
    pub cfg: Config,
    /// Bumped whenever `cfg` changes outside the user's direct edits (load, apply result).
    pub generation: u64,
}

/// Handle used by the backend to publish state and wake the window.
#[derive(Clone)]
pub struct Ui {
    pub shared: Arc<Mutex<Shared>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl Ui {
    pub fn new(cfg: Config, repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Shared { cfg, ..Default::default() };
        Self { shared: Arc::new(Mutex::new(shared)), repaint: Arc::new(repaint) }
    }

    pub fn update(&self, f: impl FnOnce(&mut Shared)) {
        f(&mut self.shared.lock().unwrap());
        (self.repaint)();
    }

    fn set_state(&self, s: UiState) {
        self.update(|sh| sh.state = s);
    }

    fn set_status(&self, text: impl Into<String>) {
        let text = text.into();
        self.update(|sh| sh.status = text);
    }

    fn progress(&self) -> Progress {
        let ui = self.clone();
        Arc::new(move |t| ui.set_status(t))
    }

    fn partial_sink(&self) -> PartialSink {
        let ui = self.clone();
        Arc::new(move |t| ui.update(|sh| sh.partial = t))
    }
}

/// Models that stay loaded for the app's lifetime.
struct Models {
    manager: Arc<FoundryLocalManager>,
    transcriber: Transcriber,
    /// Present when `keep_chat_loaded` and `summarize` are both on.
    summarizer: Option<Summarizer>,
}

/// Dictation into the meeting context: a mic capture feeding one transcription stream.
struct Dictation {
    capture: Capture,
    stream: JoinHandle<()>,
    collector: JoinHandle<()>,
}

/// A meeting in progress.
struct Active {
    captures: Vec<Capture>,
    streams: Vec<JoinHandle<()>>,
    writer: JoinHandle<(Note, Vec<String>)>,
    t0: Instant,
}

pub async fn run(cfg: Config, ui: Ui, cmds: UnboundedReceiver<Cmd>) {
    run_inner(cfg, &ui, cmds).await;
    ui.set_state(UiState::Exited);
}

async fn run_inner(mut cfg: Config, ui: &Ui, mut cmds: UnboundedReceiver<Cmd>) {
    ui.set_state(UiState::Loading);
    ui.set_status("Starting engine...");
    let manager = match transcriber::create_manager() {
        Ok(m) => m,
        Err(e) => return fail(ui, &mut cmds, format!("Engine start failed: {e}")).await,
    };
    // WinML execution providers (NPU/GPU) are discovered and registered at runtime.
    ui.set_status("Registering execution providers...");
    match manager.download_and_register_eps(None).await {
        Ok(r) => log::info!("EPs: registered=[{}] failed=[{}]", r.registered_eps.join(","), r.failed_eps.join(",")),
        Err(e) => log::warn!("EP registration failed (continuing on CPU): {e}"),
    }
    refresh_models(&manager, ui).await;

    let mut models = match load_models(&manager, &cfg, ui).await {
        Ok(m) => m,
        Err(e) => {
            log::error!("model load failed: {e:#}");
            return fail(ui, &mut cmds, format!("Model load failed: {e}")).await;
        }
    };
    refresh_models(&manager, ui).await;
    ui.update(|sh| {
        sh.cfg = cfg.clone();
        sh.generation += 1;
        sh.status = "Ready".into();
    });
    ui.set_state(UiState::Idle);

    let mut active: Option<Active> = None;
    let mut dictation: Option<Dictation> = None;
    while let Some(cmd) = cmds.recv().await {
        // Starting a meeting (or quitting) ends any dictation first, so the mic is free.
        if matches!(cmd, Cmd::Toggle | Cmd::Quit) {
            if let Some(d) = dictation.take() {
                stop_dictation(d, ui).await;
            }
        }
        match cmd {
            Cmd::Dictate if active.is_none() => match dictation.take() {
                Some(d) => {
                    stop_dictation(d, ui).await;
                    ui.set_status("Ready");
                }
                None => match start_dictation(&models, ui) {
                    Ok(d) => {
                        dictation = Some(d);
                        ui.update(|sh| {
                            sh.partial.clear();
                            sh.dictating = true;
                            sh.status = "Dictating... say why this meeting is happening".into();
                        });
                    }
                    Err(e) => {
                        log::error!("dictation failed to start: {e:#}");
                        ui.set_status(format!("Could not start dictation: {e}"));
                    }
                },
            },
            Cmd::Toggle => match active.take() {
                None => match start_devices(&cfg, &models, ui) {
                    Ok(a) => {
                        active = Some(a);
                        ui.update(|sh| {
                            sh.recent.clear();
                            sh.partial.clear();
                            sh.status = "Recording...".into();
                            sh.state = UiState::Recording;
                        });
                    }
                    Err(e) => {
                        log::error!("start failed: {e:#}");
                        ui.set_status(format!("Could not start: {e}"));
                    }
                },
                Some(a) => {
                    finish(&cfg, &models, a, ui).await;
                    if drain_pending(&mut cmds) {
                        return;
                    }
                }
            },
            Cmd::Quit => {
                if let Some(a) = active.take() {
                    finish(&cfg, &models, a, ui).await;
                }
                return;
            }
            Cmd::ApplyModels { speech, chat } if active.is_none() && dictation.is_none() => {
                ui.set_state(UiState::Loading);
                if let Err(e) = apply_models(&mut cfg, &mut models, ui, speech, chat).await {
                    log::error!("applying models failed: {e:#}");
                    ui.set_status(format!("Model change failed: {e}"));
                } else {
                    ui.set_status("Ready");
                }
                refresh_models(&models.manager, ui).await;
                ui.update(|sh| {
                    sh.cfg = cfg.clone();
                    sh.generation += 1;
                });
                ui.set_state(UiState::Idle);
            }
            Cmd::SetNotesDir(dir) if active.is_none() => {
                cfg.notes_dir = dir;
                if let Err(e) = config::save(&cfg) {
                    log::error!("saving config: {e:#}");
                }
                ui.update(|sh| sh.cfg = cfg.clone());
            }
            Cmd::Dictate | Cmd::ApplyModels { .. } | Cmd::SetNotesDir(_) => {}
        }
    }
}

fn start_dictation(models: &Models, ui: &Ui) -> anyhow::Result<Dictation> {
    let (tx, rx) = unbounded_channel();
    let capture = audio::start(Source::Mic, tx).context("opening the microphone")?;
    let (lines_tx, mut lines_rx) = unbounded_channel::<Line>();
    let stream = models.transcriber.spawn_stream("Dictation", rx, lines_tx, ui.partial_sink());
    // Finished utterances are appended to the context box as they arrive.
    let collector_ui = ui.clone();
    let collector = tokio::spawn(async move {
        while let Some(line) = lines_rx.recv().await {
            collector_ui.update(|sh| {
                sh.partial.clear();
                if !sh.context.is_empty() && !sh.context.ends_with(char::is_whitespace) {
                    sh.context.push(' ');
                }
                sh.context.push_str(&line.text);
            });
        }
    });
    Ok(Dictation { capture, stream, collector })
}

async fn stop_dictation(d: Dictation, ui: &Ui) {
    let Dictation { capture, stream, collector } = d;
    drop(capture); // closes the chunk channel -> the stream flushes its last words and ends
    let _ = stream.await;
    let _ = collector.await;
    ui.update(|sh| {
        sh.dictating = false;
        sh.partial.clear();
    });
}

/// Show an unrecoverable startup error and wait for the window to ask to quit.
async fn fail(ui: &Ui, cmds: &mut UnboundedReceiver<Cmd>, msg: String) {
    ui.set_status(msg);
    ui.set_state(UiState::Error);
    while let Some(c) = cmds.recv().await {
        if matches!(c, Cmd::Quit) {
            break;
        }
    }
}

/// Drop toggles that piled up while summarizing; returns true if a Quit was among them.
fn drain_pending(cmds: &mut UnboundedReceiver<Cmd>) -> bool {
    while let Ok(c) = cmds.try_recv() {
        if matches!(c, Cmd::Quit) {
            return true;
        }
    }
    false
}

/// Nemotron streams live; Whisper is file-based and runs on pause-delimited segments
/// (see whisper.rs). Other speech models in the catalog are not wired up.
fn is_supported_speech(alias: &str) -> bool {
    alias.starts_with("nemotron") || crate::whisper::is_whisper(alias)
}

async fn refresh_models(manager: &FoundryLocalManager, ui: &Ui) {
    let all = match manager.catalog().get_models().await {
        Ok(m) => m,
        Err(e) => {
            log::warn!("listing catalog: {e}");
            return;
        }
    };
    let (mut speech, mut chat) = (Vec::new(), Vec::new());
    for m in all {
        let Ok(info) = m.info() else { continue };
        let entry = ModelEntry { alias: m.alias().to_string(), size_mb: info.file_size_mb, cached: info.cached };
        match info.task.as_deref() {
            Some("automatic-speech-recognition") if is_supported_speech(&entry.alias) => speech.push(entry),
            Some("chat-completion") => chat.push(entry),
            _ => {}
        }
    }
    for list in [&mut speech, &mut chat] {
        list.sort_by_key(|e| (e.size_mb.is_none(), e.size_mb, e.alias.clone()));
    }
    ui.update(|sh| {
        sh.speech_models = speech;
        sh.chat_models = chat;
    });
}

async fn load_models(manager: &Arc<FoundryLocalManager>, cfg: &Config, ui: &Ui) -> anyhow::Result<Models> {
    let progress = ui.progress();
    let transcriber = Transcriber::load(manager, &cfg.speech_model, cfg.speech_variant.as_deref(), &progress)
        .await
        .context("speech model")?;
    let summarizer = if cfg.summarize && cfg.keep_chat_loaded {
        Some(
            Summarizer::load(manager, &cfg.chat_model, cfg.chat_variant.as_deref(), &progress)
                .await
                .context("chat model")?,
        )
    } else {
        None
    };
    Ok(Models { manager: manager.clone(), transcriber, summarizer })
}

/// Switch to the selected models (downloading them from Foundry Local if needed). The old model
/// stays loaded until the new one is ready, so a failed download changes nothing.
async fn apply_models(
    cfg: &mut Config,
    models: &mut Models,
    ui: &Ui,
    speech: String,
    chat: String,
) -> anyhow::Result<()> {
    let progress = ui.progress();
    let mut next = cfg.clone();
    if speech != cfg.speech_model {
        let new = Transcriber::load(&models.manager, &speech, None, &progress).await?;
        let old = std::mem::replace(&mut models.transcriber, new);
        old.unload().await;
        next.speech_model = speech;
        next.speech_variant = None; // a pinned variant belongs to the old alias
    }
    if chat != cfg.chat_model {
        let new = Summarizer::load(&models.manager, &chat, None, &progress).await?;
        if cfg.summarize && cfg.keep_chat_loaded {
            if let Some(old) = models.summarizer.replace(new) {
                old.unload().await;
            }
        } else {
            new.unload().await; // downloaded; loaded on demand at summary time
        }
        next.chat_model = chat;
        next.chat_variant = None;
    }
    config::save(&next)?;
    *cfg = next;
    Ok(())
}

fn start_devices(cfg: &Config, models: &Models, ui: &Ui) -> anyhow::Result<Active> {
    if cfg.loopback_device == LoopbackDevice::Communications {
        log::warn!("loopback_device=communications is not supported yet; using the default device");
    }
    let mut captures = Vec::new();
    let open = |source: Source, captures: &mut Vec<Capture>| {
        let (tx, rx) = unbounded_channel();
        match audio::start(source, tx) {
            Ok(c) => {
                captures.push(c);
                Some(rx)
            }
            Err(e) => {
                log::error!("{source:?} capture unavailable: {e:#}");
                None
            }
        }
    };
    let mic = open(Source::Mic, &mut captures);
    let loopback = open(Source::Loopback, &mut captures);
    let mut inputs = Vec::new();
    match cfg.capture_mode {
        CaptureMode::Dual => {
            inputs.extend(mic.map(|rx| ("Me", rx)));
            inputs.extend(loopback.map(|rx| ("Them", rx)));
        }
        CaptureMode::Mixed => {
            let rx = match (mic, loopback) {
                (Some(a), Some(b)) => Some(audio::mix(a, b)),
                (a, b) => a.or(b),
            };
            inputs.extend(rx.map(|rx| ("Meeting", rx)));
        }
    }
    anyhow::ensure!(!inputs.is_empty(), "no audio device could be opened");
    start_pipeline(cfg, models, ui, captures, inputs)
}

fn start_pipeline(
    cfg: &Config,
    models: &Models,
    ui: &Ui,
    captures: Vec<Capture>,
    inputs: Vec<(&'static str, UnboundedReceiver<Vec<i16>>)>,
) -> anyhow::Result<Active> {
    let started = Local::now();
    let mut note = Note::create(&cfg.notes_dir, started)?;
    log::info!("recording to {}", note.path.display());
    ui.update(|sh| sh.last_note = Some(note.path.clone()));

    let (lines_tx, mut lines_rx) = unbounded_channel::<Line>();
    let writer_ui = ui.clone();
    let writer = tokio::spawn(async move {
        let mut transcript = Vec::new();
        while let Some(line) = lines_rx.recv().await {
            if let Err(e) = note.append(&line) {
                log::error!("writing transcript line: {e:#}");
            }
            transcript.push(format!("{}: {}", line.label, line.text));
            let shown = line.markdown().trim_start_matches("- ").replace("**", "");
            writer_ui.update(|sh| {
                sh.partial.clear();
                sh.recent.push(shown);
                let excess = sh.recent.len().saturating_sub(12);
                sh.recent.drain(..excess);
            });
        }
        (note, transcript)
    });
    let streams = inputs
        .into_iter()
        .map(|(label, rx)| models.transcriber.spawn_stream(label, rx, lines_tx.clone(), ui.partial_sink()))
        .collect();
    Ok(Active { captures, streams, writer, t0: Instant::now() })
}

async fn finish(cfg: &Config, models: &Models, a: Active, ui: &Ui) {
    ui.set_state(UiState::Summarizing);
    ui.set_status("Finishing transcript...");
    match stop_and_write(cfg, models, a, ui).await {
        Ok(()) => ui.set_status("Saved"),
        Err(e) => {
            log::error!("finishing meeting failed: {e:#}");
            ui.set_status(format!("Save failed: {e}"));
        }
    }
    ui.set_state(UiState::Idle);
}

/// Stop capture, let the engine flush, summarize, and rewrite the note.
async fn stop_and_write(cfg: &Config, models: &Models, a: Active, ui: &Ui) -> anyhow::Result<()> {
    let Active { captures, streams, writer, t0 } = a;
    drop(captures); // closes the chunk channels -> stream tasks drain and end
    for s in streams {
        let _ = s.await;
    }
    let (note, transcript) = writer.await.context("transcript writer")?;
    let duration = t0.elapsed();
    let context = ui.shared.lock().unwrap().context.trim().to_string();

    let summary = if !cfg.summarize {
        String::new()
    } else if transcript.is_empty() {
        "## Summary\n(no speech detected)".to_string()
    } else {
        ui.set_status("Summarizing...");
        match summarize(cfg, models, ui, &transcript.join("\n"), &context).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("summary failed: {e:#}");
                format!("## Summary\n(summary unavailable: {e})")
            }
        }
    };
    let path = note.path.clone();
    note.finish(duration, &context, &summary)?;
    log::info!("saved {} ({}s)", path.display(), duration.as_secs());
    // The context belonged to this meeting; the next one starts blank.
    ui.update(|sh| {
        sh.last_note = Some(path);
        sh.context.clear();
    });
    Ok(())
}

async fn summarize(
    cfg: &Config,
    models: &Models,
    ui: &Ui,
    transcript: &str,
    context: &str,
) -> anyhow::Result<String> {
    if let Some(s) = &models.summarizer {
        return s.summarize(transcript, context).await;
    }
    // keep_chat_loaded = false: load on demand, unload afterwards.
    let s = Summarizer::load(&models.manager, &cfg.chat_model, cfg.chat_variant.as_deref(), &ui.progress()).await?;
    let out = s.summarize(transcript, context).await;
    s.unload().await;
    out
}

/// End-to-end check without a window or audio devices: feed two WAVs (16 kHz mono 16-bit) in
/// real time as "Me"/"Them", then stop, summarize and write the note.
pub async fn selftest(cfg: Config, me: &Path, them: &Path) -> anyhow::Result<()> {
    let ui = Ui::new(cfg.clone(), || {});
    let manager = transcriber::create_manager()?;
    let _ = manager.download_and_register_eps(None).await;
    let models = load_models(&manager, &cfg, &ui).await?;
    let mut inputs = Vec::new();
    let mut feeders = Vec::new();
    for (label, path) in [("Me", me), ("Them", them)] {
        let samples: Vec<i16> = hound::WavReader::open(path)?.samples::<i16>().collect::<Result<_, _>>()?;
        let (tx, rx) = unbounded_channel();
        feeders.push(tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            for chunk in samples.chunks(1600) {
                tick.tick().await;
                let _ = tx.send(chunk.to_vec());
            }
        }));
        inputs.push((label, rx));
    }
    let active = start_pipeline(&cfg, &models, &ui, Vec::new(), inputs)?;
    for f in feeders {
        f.await?;
    }
    stop_and_write(&cfg, &models, active, &ui).await
}
