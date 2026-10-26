//! Model lifecycle and live transcription sessions.
//!
//! SDK 2.x streaming (see docs/FINDINGS.md): one `AudioSession` per audio source on a single
//! loaded model. The first request item is an AUDIO item (format "pcm"), later chunks are pushed
//! to the input queue as BYTES. The engine emits sub-word text deltas with no final/partial
//! marker, so utterances are cut here: at sentence-ending punctuation, or after an idle gap.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use foundry_local_sdk::{
    AudioSession, FoundryLocalConfig, FoundryLocalManager, Item, Model, Request,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;

use crate::audio::SAMPLE_RATE;
use crate::notes::Line;

/// Cut an utterance if the engine has been silent this long.
const IDLE_FLUSH: Duration = Duration::from_millis(1500);
/// Throttle for live partial text (tray tooltip).
const PARTIAL_EVERY: Duration = Duration::from_millis(500);

pub type PartialSink = Arc<dyn Fn(String) + Send + Sync>;
/// Status text for the UI ("Downloading ... 40%") and the download fraction (0..=1) while one runs.
pub type Progress = Arc<dyn Fn(String, Option<f32>) + Send + Sync>;

/// Resolve an alias (optionally pinning a variant), download if needed, and load it.
pub async fn load_model(
    manager: &FoundryLocalManager,
    alias: &str,
    variant: Option<&str>,
    progress: &Progress,
) -> anyhow::Result<Arc<Model>> {
    let model = manager.catalog().get_model(alias).await?;
    if let Some(v) = variant.filter(|v| !v.trim().is_empty()) {
        model.select_variant_by_id(v)?;
    }
    if !model.is_cached().await? {
        log::info!("downloading {alias}");
        progress(format!("Downloading {alias}..."), Some(0.0));
        let (name, progress) = (alias.to_string(), progress.clone());
        let mut last = -5.0;
        model
            .download(Some(move |p: f64| {
                progress(format!("Downloading {name}: {p:.0}%"), Some((p / 100.0).clamp(0.0, 1.0) as f32));
                if p - last >= 10.0 {
                    last = p;
                    log::info!("  {name}: {p:.0}%");
                }
            }))
            .await?;
    }
    progress(format!("Loading {alias}..."), None);
    let t = Instant::now();
    model.load().await?;
    log::info!("{alias} ({}) loaded in {:.1}s", model.id(), t.elapsed().as_secs_f64());
    Ok(model)
}

pub fn create_manager() -> anyhow::Result<Arc<FoundryLocalManager>> {
    let dir = crate::config::app_dir();
    let cfg = FoundryLocalConfig::new("mynotetaker")
        .app_data_dir(dir.to_string_lossy().to_string())
        .logs_dir(dir.join("foundry-logs").to_string_lossy().to_string());
    Ok(FoundryLocalManager::create(cfg)?)
}

/// The resident speech model.
pub struct Transcriber {
    pub model: Arc<Model>,
}

impl Transcriber {
    pub async fn load(
        manager: &FoundryLocalManager,
        alias: &str,
        variant: Option<&str>,
        progress: &Progress,
    ) -> anyhow::Result<Self> {
        Ok(Self { model: load_model(manager, alias, variant, progress).await? })
    }

    pub async fn unload(&self) {
        if let Err(e) = self.model.unload().await {
            log::warn!("unloading speech model: {e}");
        }
    }

    /// Transcribe one audio source until `rx` closes; finished utterances go to `lines`.
    pub fn spawn_stream(
        &self,
        label: &'static str,
        rx: UnboundedReceiver<Vec<i16>>,
        lines: UnboundedSender<Line>,
        partial: PartialSink,
    ) -> tokio::task::JoinHandle<()> {
        let model = self.model.clone();
        let whisper = crate::whisper::is_whisper(model.alias());
        tokio::spawn(async move {
            let result = if whisper {
                crate::whisper::run_stream(&model, label, rx, &lines, &partial).await
            } else {
                run_stream(&model, label, rx, &lines, &partial).await
            };
            if let Err(e) = result {
                log::error!("{label} stream failed: {e:#}");
            }
        })
    }
}

fn pcm_bytes(chunk: &[i16]) -> Vec<u8> {
    chunk.iter().flat_map(|s| s.to_le_bytes()).collect()
}

async fn run_stream(
    model: &Model,
    label: &'static str,
    mut rx: UnboundedReceiver<Vec<i16>>,
    lines: &UnboundedSender<Line>,
    partial: &PartialSink,
) -> anyhow::Result<()> {
    // The engine needs the first audio up front, so the session starts on the first chunk.
    let Some(first) = rx.recv().await else {
        return Ok(());
    };
    let session = AudioSession::new(model).await?;
    let queue = session.create_input_queue()?;
    let audio = Item::audio_data(pcm_bytes(&first), Some("pcm"), SAMPLE_RATE as i32, 1);
    let mut stream = session
        .process_streaming_request(Request::from_items(vec![audio]).with_input_queue(queue.clone()));

    let feeder = tokio::spawn(async move {
        while let Some(chunk) = rx.recv().await {
            if queue.push(&Item::bytes(pcm_bytes(&chunk))).is_err() {
                break;
            }
        }
        queue.mark_finished();
    });

    let mut utt = Utterance::default();
    let mut last_partial = Instant::now();
    loop {
        match tokio::time::timeout(IDLE_FLUSH, stream.next()).await {
            Err(_) => utt.flush(label, lines),
            Ok(None) => break,
            Ok(Some(Err(e))) => {
                log::error!("{label} engine error: {e}");
                break;
            }
            Ok(Some(Ok(Item::SpeechSegment(seg)))) => {
                utt.push(&seg.text);
                if utt.ends_sentence() {
                    utt.flush(label, lines);
                } else if last_partial.elapsed() >= PARTIAL_EVERY {
                    last_partial = Instant::now();
                    partial(format!("{label}: {}", utt.text.trim()));
                }
            }
            Ok(Some(Ok(_))) => {}
        }
    }
    utt.flush(label, lines);
    let _ = feeder.await;
    Ok(())
}

#[derive(Default)]
struct Utterance {
    text: String,
    started: Option<DateTime<Local>>,
}

impl Utterance {
    fn push(&mut self, delta: &str) {
        if self.started.is_none() && !delta.trim().is_empty() {
            self.started = Some(Local::now());
        }
        self.text.push_str(delta);
    }

    fn ends_sentence(&self) -> bool {
        self.text.trim_end().ends_with(['.', '?', '!'])
    }

    fn flush(&mut self, label: &'static str, lines: &UnboundedSender<Line>) {
        let text = self.text.trim().to_string();
        let at = self.started.take().unwrap_or_else(Local::now);
        self.text.clear();
        // The engine often emits the closing "." after an idle flush; don't make it a line.
        if text.chars().any(|c| c.is_alphanumeric()) {
            let _ = lines.send(Line { at, label, text });
        }
    }
}
