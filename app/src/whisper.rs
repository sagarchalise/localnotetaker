//! Whisper path: the SDK only transcribes Whisper from complete audio, so the live stream is cut
//! into utterance-sized WAV segments at pauses (energy-based voice activity detection) and each
//! segment is transcribed as it ends. Text therefore appears a few seconds after a pause rather
//! than word by word.

use std::collections::VecDeque;
use std::io::Cursor;

use chrono::{DateTime, Duration as ChronoDuration, Local};
use foundry_local_sdk::{AudioSession, Item, Model};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::audio::SAMPLE_RATE;
use crate::notes::Line;
use crate::transcriber::PartialSink;

const FRAME: usize = (SAMPLE_RATE as usize) / 50; // 20 ms
const PREROLL_FRAMES: usize = 10; // 200 ms kept before speech starts
const END_SILENCE_FRAMES: usize = 35; // 700 ms of quiet ends a segment
const MAX_SEGMENT_SAMPLES: usize = 25 * SAMPLE_RATE as usize;
const MIN_SPEECH_SAMPLES: usize = (SAMPLE_RATE as usize) * 3 / 10;
/// Absolute RMS floor (of 32767) below which a frame is never speech.
const MIN_RMS: f32 = 300.0;

pub fn is_whisper(alias: &str) -> bool {
    alias.starts_with("whisper")
}

struct Segment {
    start: DateTime<Local>,
    samples: Vec<i16>,
}

/// Splits a 16 kHz mono stream into speech segments.
struct Segmenter {
    pending: Vec<i16>,
    preroll: VecDeque<Vec<i16>>,
    in_speech: bool,
    buf: Vec<i16>,
    silent_frames: usize,
    started: DateTime<Local>,
    /// Running estimate of the background noise level.
    noise: f32,
}

impl Segmenter {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            preroll: VecDeque::new(),
            in_speech: false,
            buf: Vec::new(),
            silent_frames: 0,
            started: Local::now(),
            noise: MIN_RMS / 3.0,
        }
    }

    fn push(&mut self, chunk: &[i16]) -> Vec<Segment> {
        self.pending.extend_from_slice(chunk);
        let mut out = Vec::new();
        while self.pending.len() >= FRAME {
            let frame: Vec<i16> = self.pending.drain(..FRAME).collect();
            let rms = (frame.iter().map(|&s| (s as f32).powi(2)).sum::<f32>() / FRAME as f32).sqrt();
            let speech = rms > (self.noise * 3.0).max(MIN_RMS);
            if !self.in_speech {
                if speech {
                    self.in_speech = true;
                    self.silent_frames = 0;
                    let lead = self.preroll.len() as i64 * 20;
                    self.started = Local::now() - ChronoDuration::milliseconds(lead + 20);
                    self.buf = self.preroll.drain(..).flatten().collect();
                    self.buf.extend_from_slice(&frame);
                } else {
                    self.noise = self.noise * 0.95 + rms * 0.05;
                    self.preroll.push_back(frame);
                    if self.preroll.len() > PREROLL_FRAMES {
                        self.preroll.pop_front();
                    }
                }
            } else {
                self.buf.extend_from_slice(&frame);
                self.silent_frames = if speech { 0 } else { self.silent_frames + 1 };
                if self.silent_frames >= END_SILENCE_FRAMES || self.buf.len() >= MAX_SEGMENT_SAMPLES {
                    out.extend(self.cut());
                }
            }
        }
        out
    }

    /// Emit the segment in progress (if it holds enough speech) and reset.
    fn cut(&mut self) -> Option<Segment> {
        self.in_speech = false;
        let speech_len = self.buf.len().saturating_sub(self.silent_frames * FRAME);
        let samples = std::mem::take(&mut self.buf);
        self.silent_frames = 0;
        (speech_len >= MIN_SPEECH_SAMPLES).then_some(Segment { start: self.started, samples })
    }

    fn finish(&mut self) -> Option<Segment> {
        if self.in_speech {
            self.cut()
        } else {
            None
        }
    }
}

fn wav_bytes(samples: &[i16]) -> anyhow::Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec)?;
        for s in samples {
            w.write_sample(*s)?;
        }
        w.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// Whisper emits markers like "[BLANK_AUDIO]" or "(silence)" for non-speech; drop those.
fn is_non_speech(text: &str) -> bool {
    let t = text.trim();
    t.is_empty()
        || (t.starts_with('[') && t.ends_with(']'))
        || (t.starts_with('(') && t.ends_with(')'))
        || !t.chars().any(|c| c.is_alphanumeric())
}

pub async fn run_stream(
    model: &Model,
    label: &'static str,
    mut rx: UnboundedReceiver<Vec<i16>>,
    lines: &UnboundedSender<Line>,
    partial: &PartialSink,
) -> anyhow::Result<()> {
    let session = AudioSession::new(model).await?;
    let (seg_tx, mut seg_rx) = unbounded_channel::<Segment>();

    // One worker keeps segments in order and decouples transcription time from capture.
    let (lines, partial) = (lines.clone(), partial.clone());
    let temp_dir = crate::config::app_dir().join("tmp");
    std::fs::create_dir_all(&temp_dir)?;
    let worker = tokio::spawn(async move {
        let mut n = 0u32;
        while let Some(seg) = seg_rx.recv().await {
            partial(format!("{label}: transcribing..."));
            // The engine only accepts a file path for non-streaming audio. The segment is
            // written to a temp WAV and deleted as soon as it has been transcribed.
            let path = temp_dir.join(format!("{label}-{n}.wav"));
            n += 1;
            let wrote = wav_bytes(&seg.samples).and_then(|b| Ok(std::fs::write(&path, b)?));
            if let Err(e) = wrote {
                log::error!("{label}: writing temp wav: {e}");
                continue;
            }
            let audio = Item::audio_uri(path.to_string_lossy(), Some("wav"), SAMPLE_RATE as i32, 1);
            let result = session.transcribe(audio).await;
            let _ = std::fs::remove_file(&path);
            match result {
                Ok(text) if !is_non_speech(&text) => {
                    let _ = lines.send(Line { at: seg.start, label, text: text.trim().to_string() });
                }
                Ok(_) => {}
                Err(e) => log::error!("{label}: whisper transcription failed: {e}"),
            }
            partial(String::new());
        }
    });

    let mut segmenter = Segmenter::new();
    while let Some(chunk) = rx.recv().await {
        for seg in segmenter.push(&chunk) {
            let _ = seg_tx.send(seg);
        }
    }
    if let Some(seg) = segmenter.finish() {
        let _ = seg_tx.send(seg);
    }
    drop(seg_tx);
    let _ = worker.await;
    Ok(())
}
