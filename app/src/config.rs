//! `%APPDATA%\MyNoteTaker\config.toml`.

use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    /// Mic and loopback in separate sessions, labeled Me / Them.
    Dual,
    /// Both summed into one session, labeled Meeting.
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopbackDevice {
    Default,
    Communications,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: PathBuf,
    pub speech_model: String,
    pub speech_variant: Option<String>,
    pub language: String,
    pub chat_model: String,
    pub chat_variant: Option<String>,
    pub summarize: bool,
    pub keep_chat_loaded: bool,
    pub hotkey: String,
    /// Toggles dictation into the meeting-context box (only while not recording).
    pub dictate_hotkey: String,
    pub capture_mode: CaptureMode,
    pub loopback_device: LoopbackDevice,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            notes_dir: home.join("Documents").join("MyNoteTaker"),
            speech_model: "nemotron-speech-streaming-en-0.6b".into(),
            speech_variant: None,
            language: "en".into(),
            // A few-GB model: handles long transcripts and a meeting-context prompt far better
            // than the 0.5B class. It has CPU, GPU and Intel NPU variants.
            chat_model: "phi-4-mini".into(),
            chat_variant: None,
            summarize: true,
            // Loaded at summary time and unloaded after, so the model's RAM is not held all day.
            keep_chat_loaded: false,
            hotkey: "Ctrl+Alt+N".into(),
            dictate_hotkey: "Ctrl+Alt+D".into(),
            capture_mode: CaptureMode::Dual,
            loopback_device: LoopbackDevice::Default,
        }
    }
}

const TEMPLATE: &str = r#"# MyNoteTaker configuration
# notes_dir = "C:\\Users\\you\\Documents\\MyNoteTaker"
speech_model = "nemotron-speech-streaming-en-0.6b"
# speech_variant = "nemotron-speech-streaming-en-0.6b-generic-cpu:3"
language = "en"

# phi-4-mini has CPU, GPU and Intel NPU variants. Smaller: qwen2.5-1.5b or qwen2.5-0.5b (<1 GB).
# Pin a variant to force a device, e.g. the Intel NPU:
#   chat_variant = "phi-4-mini-instruct-openvino-npu:4"
chat_model = "phi-4-mini"
# chat_variant = ""
summarize = true
keep_chat_loaded = false       # true keeps the chat model in RAM between meetings

hotkey = "Ctrl+Alt+N"
dictate_hotkey = "Ctrl+Alt+D"  # toggle dictation of the meeting context (when not recording)
capture_mode = "dual"          # dual | mixed
loopback_device = "default"    # default | communications (communications not yet supported)
"#;

pub fn app_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MyNoteTaker")
}

/// Persist the config (comments from the first-run template are not preserved).
pub fn save(cfg: &Config) -> anyhow::Result<()> {
    let text = format!(
        "# MyNoteTaker configuration (written by the app)\n{}",
        toml::to_string_pretty(cfg).context("serializing config")?
    );
    std::fs::write(app_dir().join("config.toml"), text).context("writing config.toml")
}

pub fn load() -> anyhow::Result<Config> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir).context("creating config dir")?;
    let path = dir.join("config.toml");
    if !path.exists() {
        std::fs::write(&path, TEMPLATE).context("writing default config")?;
    }
    let text = std::fs::read_to_string(&path).context("reading config.toml")?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
