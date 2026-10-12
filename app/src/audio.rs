//! Audio capture: mic and WASAPI loopback (via cpal) -> 16 kHz mono 16-bit PCM chunks.
//!
//! `cpal::Stream` is not `Send`, so each capture owns a thread that builds the stream, keeps it
//! alive until told to stop, then drops it (which closes the chunk channel).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub const SAMPLE_RATE: u32 = 16_000;
/// How often a capture checks whether the default device changed.
const DEVICE_POLL: Duration = Duration::from_secs(1);
/// Chunk the mixer emits per tick.
const MIX_CHUNK: usize = (SAMPLE_RATE as usize) / 10;

#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// Default input device.
    Mic,
    /// Default output device captured as WASAPI loopback.
    Loopback,
}

pub type Chunks = UnboundedSender<Vec<i16>>;

/// A running capture. Dropping it stops the capture and closes its channel.
pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start(source: Source, tx: Chunks) -> anyhow::Result<Capture> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel::<anyhow::Result<()>>();
    let stop_t = stop.clone();
    let thread = std::thread::Builder::new()
        .name(format!("capture-{source:?}"))
        .spawn(move || {
            let broken = Arc::new(AtomicBool::new(false));
            let mut name = default_device_name(source);
            let mut stream = match build_stream(source, tx.clone(), broken.clone()) {
                Ok(s) => {
                    let _ = ready_tx.send(Ok(()));
                    Some(s)
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            // Bluetooth headsets re-route when a call opens the mic (A2DP -> Hands-Free), when
            // they connect, and when they drop. Watch the default device and the stream's
            // error flag, and rebuild the stream on the same channel so the session carries on.
            let mut last_check = Instant::now();
            let mut failing = false;
            while !stop_t.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
                if last_check.elapsed() < DEVICE_POLL && !broken.load(Ordering::Relaxed) {
                    continue;
                }
                last_check = Instant::now();
                let now = default_device_name(source);
                if broken.swap(false, Ordering::Relaxed) || now != name || stream.is_none() {
                    if !failing {
                        log::warn!("{source:?}: device change/error ({name:?} -> {now:?}); reopening");
                    }
                    drop(stream.take());
                    match build_stream(source, tx.clone(), broken.clone()) {
                        Ok(s) => {
                            stream = Some(s);
                            name = now;
                            failing = false;
                            log::info!("{source:?}: reopened");
                        }
                        Err(e) => {
                            if !failing {
                                log::warn!("{source:?}: reopen failed, retrying every {DEVICE_POLL:?}: {e:#}");
                            }
                            failing = true;
                        }
                    }
                }
            }
            drop(stream);
        })?;
    ready_rx.recv().context("capture thread died")??;
    Ok(Capture { stop, thread: Some(thread) })
}

fn default_device_name(source: Source) -> Option<String> {
    let host = cpal::default_host();
    match source {
        Source::Mic => host.default_input_device(),
        Source::Loopback => host.default_output_device(),
    }
    .and_then(|d| d.name().ok())
}

fn build_stream(source: Source, tx: Chunks, broken: Arc<AtomicBool>) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();
    let (device, config) = match source {
        Source::Mic => {
            let d = host.default_input_device().context("no default input device (mic)")?;
            let c = d.default_input_config()?;
            (d, c)
        }
        Source::Loopback => {
            let d = host.default_output_device().context("no default output device")?;
            let c = d.default_output_config()?;
            (d, c)
        }
    };
    log::info!(
        "{source:?}: {} ({} ch, {} Hz, {:?})",
        device.name().unwrap_or_default(),
        config.channels(),
        config.sample_rate().0,
        config.sample_format()
    );
    let (rate, channels, format) = (config.sample_rate().0, config.channels() as usize, config.sample_format());
    let cfg = config.into();
    let stream = match format {
        SampleFormat::F32 => input_stream::<f32>(&device, &cfg, rate, channels, tx, source, broken)?,
        SampleFormat::I16 => input_stream::<i16>(&device, &cfg, rate, channels, tx, source, broken)?,
        SampleFormat::U16 => input_stream::<u16>(&device, &cfg, rate, channels, tx, source, broken)?,
        f => bail!("unsupported sample format {f:?}"),
    };
    stream.play()?;
    Ok(stream)
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rate: u32,
    channels: usize,
    tx: Chunks,
    source: Source,
    broken: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut resampler = Resampler::new(rate);
    let mut mono = Vec::new();
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            mono.clear();
            mono.extend(data.chunks(channels).map(|f| {
                f.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32
            }));
            let out = resampler.push(&mono);
            if !out.is_empty() {
                let _ = tx.send(out);
            }
        },
        move |e| {
            log::error!("{source:?} stream error: {e}");
            broken.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}

/// Streaming box-filter resampler to 16 kHz mono i16. Adequate for speech recognition; swap for
/// a polyphase resampler (rubato) if recognition quality on loopback turns out to need it.
pub struct Resampler {
    ratio: f64,
    buf: Vec<f32>,
    /// Start of the next output sample's window, in `buf` coordinates.
    next: f64,
}

impl Resampler {
    pub fn new(rate: u32) -> Self {
        Self { ratio: rate as f64 / SAMPLE_RATE as f64, buf: Vec::new(), next: 0.0 }
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<i16> {
        self.buf.extend_from_slice(input);
        let mut out = Vec::new();
        while self.next + self.ratio <= self.buf.len() as f64 {
            let a = self.next as usize;
            let b = ((self.next + self.ratio).ceil() as usize).clamp(a + 1, self.buf.len());
            let avg = self.buf[a..b].iter().sum::<f32>() / (b - a) as f32;
            out.push((avg.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
            self.next += self.ratio;
        }
        let consumed = (self.next as usize).min(self.buf.len());
        self.buf.drain(..consumed);
        self.next -= consumed as f64;
        out
    }
}

/// Sum two chunk streams into one on a wall-clock tick. Loopback delivers nothing while the
/// system is silent, so alignment is by time, padding the missing side with silence. Ends when
/// both inputs have closed and their buffers are drained.
pub fn mix(mut a: UnboundedReceiver<Vec<i16>>, mut b: UnboundedReceiver<Vec<i16>>) -> UnboundedReceiver<Vec<i16>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let (mut qa, mut qb): (VecDeque<i16>, VecDeque<i16>) = (VecDeque::new(), VecDeque::new());
        let (mut a_open, mut b_open) = (true, true);
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            drain(&mut a, &mut qa, &mut a_open);
            drain(&mut b, &mut qb, &mut b_open);
            if !a_open && !b_open && qa.is_empty() && qb.is_empty() {
                break;
            }
            let mut out = Vec::with_capacity(MIX_CHUNK);
            for _ in 0..MIX_CHUNK {
                let s = qa.pop_front().unwrap_or(0) as i32 + qb.pop_front().unwrap_or(0) as i32;
                out.push(s.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
            }
            if tx.send(out).is_err() {
                break;
            }
        }
    });
    rx
}

fn drain(rx: &mut UnboundedReceiver<Vec<i16>>, q: &mut VecDeque<i16>, open: &mut bool) {
    use tokio::sync::mpsc::error::TryRecvError;
    loop {
        match rx.try_recv() {
            Ok(chunk) => q.extend(chunk),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                *open = false;
                break;
            }
        }
    }
}
