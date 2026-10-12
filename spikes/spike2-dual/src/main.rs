//! Spike 2: two concurrent live transcription sessions on ONE loaded streaming model.
//! Uses the SDK 2.x Session API (`AudioSession` + input `ItemQueue`); the 1.x
//! `LiveAudioTranscriptionSession` is deprecated.
//! Each "stream" is fed from a 16 kHz mono 16-bit WAV, paced in real time, standing in for
//! mic ("Me") and loopback ("Them"). Usage: spike2-dual <me.wav> <them.wav>

use std::time::{Duration, Instant};

use anyhow::Context;
use foundry_local_sdk::{
    AudioSession, FoundryLocalConfig, FoundryLocalManager, Item, ItemQueue, Request,
    SpeechSegmentKind,
};
use tokio_stream::StreamExt;

const ALIAS: &str = "nemotron-speech-streaming-en-0.6b";
const CHUNK_MS: u64 = 100;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let me = args.next().context("usage: spike2-dual <me.wav> <them.wav>")?;
    let them = args.next().context("usage: spike2-dual <me.wav> <them.wav>")?;

    let manager = FoundryLocalManager::create(FoundryLocalConfig::new("spike2_dual"))?;
    let model = manager.catalog().get_model(ALIAS).await?;
    if !model.is_cached().await? {
        println!("downloading {ALIAS} ...");
        model
            .download(Some(|p: f64| eprint!("\r  {p:.0}%  ")))
            .await?;
        eprintln!();
    }
    let t = Instant::now();
    model.load().await?;
    println!("model loaded in {:.1}s", t.elapsed().as_secs_f64());

    let start = Instant::now();
    let (a, b) = tokio::join!(
        run_stream("Me", &model, &me, start),
        run_stream("Them", &model, &them, start)
    );
    a?;
    b?;
    println!("total {:.1}s", start.elapsed().as_secs_f64());
    model.unload().await?;
    Ok(())
}

async fn run_stream(
    label: &'static str,
    model: &foundry_local_sdk::Model,
    wav: &str,
    start: Instant,
) -> anyhow::Result<()> {
    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 16_000 && spec.channels == 1 && spec.bits_per_sample == 16,
        "{wav}: need 16 kHz mono 16-bit, got {spec:?}"
    );
    let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>()?;

    let session = AudioSession::new(model).await?;
    let queue = session.create_input_queue()?;
    // Native side requires the first request item to be AUDIO (an empty request fails with
    // "First item must be AUDIO"), so the first chunk rides in the request; the rest stream
    // through the queue.
    let per_chunk = (16_000 * CHUNK_MS / 1000) as usize;
    let (first, rest) = samples.split_at(per_chunk.min(samples.len()));
    let mut stream = session.process_streaming_request(
        Request::from_items(vec![pcm_item(first)]).with_input_queue(queue.clone()),
    );

    let feeder = tokio::spawn(feed(queue, rest.to_vec()));

    while let Some(item) = stream.next().await {
        match item? {
            Item::SpeechSegment(seg) => {
                let kind = match seg.kind {
                    SpeechSegmentKind::Final => "FINAL  ",
                    SpeechSegmentKind::Partial => "partial",
                    SpeechSegmentKind::None => "none   ",
                };
                println!("[{:6.2}s] {label:<4} {kind} {}", start.elapsed().as_secs_f64(), seg.text);
            }
            Item::SpeechResult(r) => {
                println!("[{:6.2}s] {label:<4} RESULT  {}", start.elapsed().as_secs_f64(), r.text);
            }
            other => println!("[{label}] other item: {other:?}"),
        }
    }
    feeder.await??;
    let resp = stream.response().await?;
    println!("[{label}] done: {:?}", resp.finish_reason);
    Ok(())
}

/// Push 100 ms PCM16 chunks at real-time pace, then signal end of audio.
async fn feed(queue: ItemQueue, samples: Vec<i16>) -> anyhow::Result<()> {
    let per_chunk = (16_000 * CHUNK_MS / 1000) as usize;
    let mut tick = tokio::time::interval(Duration::from_millis(CHUNK_MS));
    for chunk in samples.chunks(per_chunk) {
        tick.tick().await;
        // Queue items must be BYTES (the AUDIO item only describes the format up front).
        queue.push(&Item::bytes(pcm_bytes(chunk)))?;
    }
    queue.mark_finished();
    Ok(())
}

fn pcm_bytes(chunk: &[i16]) -> Vec<u8> {
    chunk.iter().flat_map(|s| s.to_le_bytes()).collect()
}

fn pcm_item(chunk: &[i16]) -> Item {
    Item::audio_data(pcm_bytes(chunk), Some("pcm"), 16_000, 1)
}
