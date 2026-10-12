//! Spike 0: hardware and catalog check.
//! Registers execution providers, then lists every catalog variant (alias, id, device, EP)
//! and checks that the streaming speech alias resolves. Output is meant to be pasted into
//! spikes/FINDINGS.md per machine.

use foundry_local_sdk::{FoundryLocalConfig, FoundryLocalManager};

const SPEECH_ALIAS: &str = "nemotron-speech-streaming-en-0.6b";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = FoundryLocalManager::create(FoundryLocalConfig::new("spike0_catalog"))?;

    println!("== Execution providers ==");
    let eps = manager.discover_eps()?;
    for ep in &eps {
        println!("  {:<40} registered={}", ep.name, ep.is_registered);
    }

    let result = manager
        .download_and_register_eps_with_progress(None, |ep: &str, pct: f64| {
            eprint!("\r  EP {ep}: {pct:.0}%   ");
        })
        .await?;
    eprintln!();
    println!(
        "  success={} status={} registered=[{}] failed=[{}]",
        result.success,
        result.status,
        result.registered_eps.join(", "),
        result.failed_eps.join(", ")
    );

    println!("\n== Catalog (alias -> variants) ==");
    let models = manager.catalog().get_models().await?;
    println!("  {} models", models.len());
    for model in &models {
        println!("  {}", model.alias());
        for v in model.variants() {
            // SDK 2.x: info() is fallible (1.x returned the struct directly).
            let (dev, ep) = v
                .info()
                .ok()
                .and_then(|i| i.runtime)
                .map(|rt| (format!("{:?}", rt.device_type), rt.execution_provider))
                .unwrap_or_else(|| ("?".into(), "?".into()));
            let (mb, ctx) = v
                .info()
                .ok()
                .map(|i| (i.file_size_mb, i.context_length))
                .unwrap_or_default();
            println!(
                "    - {:<55} device={:<4} ep={:<26} size_mb={:?} ctx={:?}",
                v.id(),
                dev,
                ep,
                mb,
                ctx
            );
        }
    }

    println!("\n== Streaming speech model ==");
    match manager.catalog().get_model(SPEECH_ALIAS).await {
        Ok(m) => println!("  {SPEECH_ALIAS}: resolved ({} variants)", m.variants().len()),
        Err(e) => println!("  {SPEECH_ALIAS}: NOT FOUND ({e})"),
    }
    Ok(())
}
