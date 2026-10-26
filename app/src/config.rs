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

/// `accelerator` value: pick the best NPU/GPU provider for this machine.
pub const ACCEL_AUTO: &str = "auto";
/// `accelerator` value: register every execution provider this machine offers (slow first start).
pub const ACCEL_ALL: &str = "all";
/// `accelerator` value: register none; models run on CPU.
pub const ACCEL_CPU: &str = "cpu";

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
    /// "auto", "all", "cpu", or comma-separated execution provider names (e.g. "CUDAExecutionProvider").
    pub accelerator: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            notes_dir: home.join("Documents").join("MyNoteTaker"),
            speech_model: "nemotron-speech-streaming-en-0.6b".into(),
            speech_variant: None,
            language: "en".into(),
            // "auto": qwen2.5-1.5b on an NPU (885 MB), else qwen3-0.6b (593 MB CPU / 529 MB GPU).
            chat_model: crate::summarize::AUTO.into(),
            chat_variant: None,
            summarize: true,
            keep_chat_loaded: true,
            hotkey: "Ctrl+Alt+N".into(),
            dictate_hotkey: "Ctrl+Alt+D".into(),
            capture_mode: CaptureMode::Dual,
            loopback_device: LoopbackDevice::Default,
            accelerator: ACCEL_AUTO.into(),
        }
    }
}

const TEMPLATE: &str = r#"# MyNoteTaker configuration
# notes_dir = "C:\\Users\\you\\Documents\\MyNoteTaker"
speech_model = "nemotron-speech-streaming-en-0.6b"
# speech_variant = "nemotron-speech-streaming-en-0.6b-generic-cpu:3"
language = "en"

# "auto" picks the best model under 1 GB for this machine: qwen2.5-1.5b on an NPU (885 MB),
# otherwise qwen3-0.6b (CPU/GPU, 529-593 MB). Or name any catalog model, optionally with a variant:
#   chat_model = "qwen2.5-1.5b"
#   chat_variant = "qwen2.5-1.5b-instruct-openvino-npu:5"
chat_model = "auto"
# chat_variant = ""
summarize = true
keep_chat_loaded = true        # false loads the chat model only at summary time

hotkey = "Ctrl+Alt+N"
dictate_hotkey = "Ctrl+Alt+D"  # toggle dictation of the meeting context (when not recording)
capture_mode = "dual"          # dual | mixed
loopback_device = "default"    # default | communications (communications not yet supported)

# Hardware acceleration set up at start. "auto" picks the best NPU/GPU provider for this machine;
# "all" downloads every provider on offer (slow first launch); "cpu" skips them; or name the
# ones you want, comma-separated, e.g. accelerator = "CUDAExecutionProvider"
accelerator = "auto"
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
