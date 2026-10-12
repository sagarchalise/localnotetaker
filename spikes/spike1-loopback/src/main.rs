//! Spike 1: capture system audio (WASAPI loopback) and write 16 kHz mono 16-bit PCM WAV.
//! cpal opens an *input* stream on the default *output* device, which WASAPI treats as loopback.
//! Usage: spike1-loopback [seconds=10] [out.wav=loopback.wav]
//! Note: WASAPI loopback delivers no frames while nothing is playing; play audio during capture.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

const TARGET_RATE: u32 = 16_000;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let secs: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(10);
    let out = args.next().unwrap_or_else(|| "loopback.wav".into());

    let host = cpal::default_host();
    let device = host.default_output_device().context("no default output device")?;
    println!("device: {}", device.name()?);
    let config = device.default_output_config()?;
    println!("config: {config:?}");
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let mono: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let err_fn = |e| eprintln!("stream error: {e}");
    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config.into(), channels, mono.clone(), err_fn)?,
        SampleFormat::I16 => build::<i16>(&device, &config.into(), channels, mono.clone(), err_fn)?,
        SampleFormat::U16 => build::<u16>(&device, &config.into(), channels, mono.clone(), err_fn)?,
        f => bail!("unsupported sample format {f:?}"),
    };
    stream.play()?;
    println!("capturing {secs}s - play something (e.g. a YouTube clip)...");
    std::thread::sleep(Duration::from_secs(secs));
    drop(stream);

    let native = mono.lock().unwrap();
    println!("captured {} mono frames at {rate} Hz", native.len());
    if native.is_empty() {
        bail!("no frames received (nothing playing, or loopback unsupported on this device)");
    }
    let pcm = resample_to_i16(&native, rate);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&out, spec)?;
    for s in &pcm {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    println!("wrote {out}: {} samples, peak amplitude {peak}/32767", pcm.len());
    Ok(())
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mono: Arc<Mutex<Vec<f32>>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut buf = mono.lock().unwrap();
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                buf.push(sum / channels as f32);
            }
        },
        err_fn,
        None,
    )?)
}

/// Box-filter decimation / linear upsample to 16 kHz. Good enough for a spike; the real
/// pipeline should use a proper resampler (rubato) once we confirm quality needs.
fn resample_to_i16(input: &[f32], rate: u32) -> Vec<i16> {
    let ratio = rate as f64 / TARGET_RATE as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let start = i as f64 * ratio;
            let end = ((i + 1) as f64 * ratio).min(input.len() as f64);
            let (a, b) = (start as usize, (end.ceil() as usize).max(start as usize + 1));
            let b = b.min(input.len());
            let avg = input[a..b].iter().sum::<f32>() / (b - a).max(1) as f32;
            (avg.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
        })
        .collect()
}
