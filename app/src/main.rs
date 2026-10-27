#![windows_subsystem = "windows"]

mod audio;
mod backend;
mod config;
mod hotkey;
mod notes;
mod secret;
mod summarize;
mod transcriber;
mod whisper;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use backend::{Cmd, ModelEntry, Ui, UiState};
use config::ChatProvider;

struct FileLogger(Mutex<std::fs::File>);

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            if let Ok(mut f) = self.0.lock() {
                let _ = writeln!(f, "{} {:5} {}", chrono::Local::now().format("%F %T"), r.level(), r.args());
            }
        }
    }
    fn flush(&self) {}
}

fn init_logging() {
    let dir = config::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("MyNoteTaker.log")) {
        let _ = log::set_boxed_logger(Box::new(FileLogger(Mutex::new(f))));
        log::set_max_level(log::LevelFilter::Info);
    }
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));
}

fn main() -> anyhow::Result<()> {
    init_logging();
    let mut cfg = config::load()?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("--selftest") {
        // --selftest <me.wav> <them.wav> [notes_dir]: end-to-end run with no window or devices.
        anyhow::ensure!(args.len() >= 3, "usage: --selftest <me.wav> <them.wav> [notes_dir]");
        if let Some(dir) = args.get(3) {
            cfg.notes_dir = dir.into();
        }
        // Test another speech model without touching config.toml, e.g. whisper-base.
        if let Ok(model) = std::env::var("MYNOTETAKER_SPEECH_MODEL") {
            cfg.speech_model = model;
            cfg.speech_variant = None;
        }
        let rt = tokio::runtime::Runtime::new()?;
        let result = rt.block_on(backend::selftest(cfg, args[1].as_ref(), args[2].as_ref()));
        match &result {
            Ok(()) => log::info!("selftest OK"),
            Err(e) => log::error!("selftest FAILED: {e:#}"),
        }
        return result;
    }

    let hotkey_label = cfg.hotkey.clone();
    let dictate_label = cfg.dictate_hotkey.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([540.0, 640.0]).with_title("MyNoteTaker"),
        ..Default::default()
    };
    eframe::run_native(
        "MyNoteTaker",
        options,
        Box::new(move |cc| {
            // Backend on its own tokio runtime thread; it wakes the window through `repaint`.
            let ctx = cc.egui_ctx.clone();
            let ui = Ui::new(cfg.clone(), move || ctx.request_repaint());
            let (cmd_tx, cmd_rx) = unbounded_channel::<Cmd>();
            let backend_ui = ui.clone();
            std::thread::Builder::new().name("backend".into()).spawn(move || match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(backend::run(cfg, backend_ui.clone(), cmd_rx)),
                Err(e) => {
                    log::error!("tokio runtime: {e}");
                    backend_ui.update(|s| s.state = UiState::Exited);
                }
            })?;

            // Global hotkey (must be created on the event-loop thread, i.e. here).
            let hk_manager = match GlobalHotKeyManager::new() {
                Ok(m) => {
                    let register = |spec: &str| {
                        hotkey::parse(spec)
                            .and_then(|hk| {
                                m.register(hk)?;
                                Ok(hk.id())
                            })
                            .map_err(|e| log::warn!("hotkey '{spec}' unavailable: {e:#}"))
                            .ok()
                    };
                    let toggle_id = register(&hotkey_label);
                    let dictate_id = register(&dictate_label);
                    let (tx, shared, ctx) = (cmd_tx.clone(), ui.shared.clone(), cc.egui_ctx.clone());
                    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                        if e.state != HotKeyState::Pressed {
                            return;
                        }
                        let state = shared.lock().unwrap().state;
                        let cmd = if Some(e.id) == toggle_id && matches!(state, UiState::Idle | UiState::Recording) {
                            Cmd::Toggle
                        } else if Some(e.id) == dictate_id && state == UiState::Idle {
                            Cmd::Dictate
                        } else {
                            return;
                        };
                        let _ = tx.send(cmd);
                        ctx.request_repaint();
                    }));
                    Some(m)
                }
                Err(e) => {
                    log::warn!("hotkey manager failed: {e}");
                    None
                }
            };
            Ok(Box::new(App::new(ui, cmd_tx, hk_manager, hotkey_label, dictate_label)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

struct App {
    ui: Ui,
    cmd_tx: UnboundedSender<Cmd>,
    _hotkeys: Option<GlobalHotKeyManager>,
    hotkey_label: String,
    dictate_label: String,
    speech_sel: String,
    chat_sel: String,
    provider_sel: ChatProvider,
    api_url: String,
    api_model: String,
    api_key_env: String,
    /// Key being typed; cleared as soon as it is sent.
    api_key_input: String,
    api_key_saved: bool,
    folder: String,
    /// `Shared::generation` last synced into the selections above.
    seen_generation: u64,
    closing: bool,
}

impl App {
    fn new(
        ui: Ui,
        cmd_tx: UnboundedSender<Cmd>,
        hk: Option<GlobalHotKeyManager>,
        hotkey_label: String,
        dictate_label: String,
    ) -> Self {
        Self {
            ui,
            cmd_tx,
            _hotkeys: hk,
            hotkey_label,
            dictate_label,
            speech_sel: String::new(),
            chat_sel: String::new(),
            provider_sel: ChatProvider::Foundry,
            api_url: String::new(),
            api_model: String::new(),
            api_key_env: String::new(),
            api_key_input: String::new(),
            api_key_saved: false,
            folder: String::new(),
            seen_generation: u64::MAX,
            closing: false,
        }
    }

    fn send(&self, cmd: Cmd) {
        let _ = self.cmd_tx.send(cmd);
    }
}

fn provider_label(p: ChatProvider) -> &'static str {
    match p {
        ChatProvider::Foundry => "Foundry Local (on this machine)",
        ChatProvider::OpenAi => "OpenAI-compatible endpoint",
    }
}

fn model_label(entries: &[ModelEntry], alias: &str) -> String {
    if alias == summarize::AUTO {
        return "auto  (best under 1 GB for this machine: NPU or CPU/GPU)".into();
    }
    match entries.iter().find(|e| e.alias == alias) {
        Some(e) => {
            let size = e.size_mb.map(|m| format!("{m} MB")).unwrap_or_else(|| "? MB".into());
            let state = if e.cached { "downloaded" } else { "will download" };
            let kind = if alias.starts_with("whisper") { ", transcribes after each pause" } else { "" };
            format!("{alias}  ({size}, {state}{kind})")
        }
        None => alias.to_string(),
    }
}

/// Show a folder, or open a note in its viewer. Anything else (an .exe typed into the folder box,
/// say) is refused: Explorer would run it, and this app never runs anything.
fn open_in_explorer(path: &Path) {
    let is_note = path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !(path.is_dir() || is_note) {
        log::warn!("not opening {}: not a folder or .md note", path.display());
        return;
    }
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

fn eps_label<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let labels: Vec<String> = names.into_iter().map(|n| backend::ep_label(n.trim())).collect();
    if labels.is_empty() {
        "CPU".into()
    } else {
        labels.join(" + ")
    }
}

/// `eps`: providers this machine offers (empty until the backend has discovered them).
fn accel_label(accel: &str, eps: &[String]) -> String {
    match accel {
        a if a == config::ACCEL_AUTO && eps.is_empty() => "auto  (recommended: best for this machine)".into(),
        a if a == config::ACCEL_AUTO => {
            format!("auto  (recommended: {})", eps_label(backend::auto_eps(eps)))
        }
        a if a == config::ACCEL_ALL => "all available  (slowest first launch)".into(),
        a if a == config::ACCEL_CPU => "CPU only  (fastest start, slower models)".into(),
        list => eps_label(list.split(',')),
    }
}

fn mm_ss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Dropdown over catalog entries; returns true if the user picked a different model.
fn model_combo(ui: &mut egui::Ui, id: &str, entries: &[ModelEntry], selected: &mut String, default: &str) -> bool {
    let before = selected.clone();
    let filter_id = egui::Id::new((id, "model-filter"));
    let shown =
        egui::ComboBox::from_id_salt(id).width(400.0).selected_text(model_label(entries, selected)).show_ui(ui, |ui| {
            let mut filter = ui.data_mut(|d| d.get_temp::<String>(filter_id)).unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search models...")).request_focus();
            let needle = filter.trim().to_lowercase();
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                let mut any = false;
                for e in entries.iter().filter(|e| e.alias.to_lowercase().contains(&needle)) {
                    any = true;
                    let mut label = model_label(entries, &e.alias);
                    if e.alias == default {
                        label.push_str("  [default]");
                    }
                    ui.selectable_value(selected, e.alias.clone(), label);
                }
                if !any {
                    ui.weak("No models match");
                }
            });
            ui.data_mut(|d| d.insert_temp(filter_id, filter));
        });
    if shown.inner.is_none() {
        // Popup closed: the next open starts with an empty search.
        ui.data_mut(|d| d.remove::<String>(filter_id));
    }
    *selected != before
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        let (
            state,
            dictating,
            status,
            download,
            hint,
            step_started,
            partial,
            recent,
            speech,
            chat,
            eps,
            last_note,
            cfg,
            generation,
        ) = {
            let s = self.ui.shared.lock().unwrap();
            (
                s.state,
                s.dictating,
                s.status.clone(),
                s.download,
                s.hint.clone(),
                s.step_started,
                s.partial.clone(),
                s.recent.clone(),
                s.speech_models.clone(),
                s.chat_models.clone(),
                s.eps.clone(),
                s.last_note.clone(),
                s.cfg.clone(),
                s.generation,
            )
        };

        // Closing while recording/summarizing: finish and save first, then close.
        if ctx.input(|i| i.viewport().close_requested()) {
            match state {
                UiState::Recording | UiState::Summarizing if !self.closing => {
                    self.closing = true;
                    self.send(Cmd::Quit);
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                UiState::Recording | UiState::Summarizing => ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose),
                _ => self.send(Cmd::Quit),
            }
        }
        if self.closing && state == UiState::Exited {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Selections follow the saved config whenever the backend reports a change.
        if self.seen_generation != generation {
            self.seen_generation = generation;
            self.speech_sel = cfg.speech_model.clone();
            self.chat_sel = cfg.chat_model.clone();
            self.provider_sel = cfg.chat_provider;
            self.api_url = cfg.chat_base_url.clone();
            self.api_model = cfg.chat_api_model.clone();
            self.api_key_env = cfg.chat_api_key_env.clone();
            self.api_key_saved = secret::has_api_key();
            self.folder = cfg.notes_dir.to_string_lossy().to_string();
        }

        let idle = state == UiState::Idle;
        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("MyNoteTaker");
            let status_text = match (state, self.closing) {
                (_, true) => "Saving the meeting before closing...".to_string(),
                (UiState::Recording, _) => "RECORDING".to_string(),
                _ => status.clone(),
            };
            let busy = matches!(state, UiState::Loading | UiState::Summarizing) || self.closing;
            let color = match state {
                UiState::Recording => egui::Color32::from_rgb(220, 50, 50),
                UiState::Error => egui::Color32::from_rgb(220, 120, 40),
                UiState::Loading => egui::Color32::from_rgb(70, 140, 230),
                _ => ui.visuals().text_color(),
            };
            ui.horizontal(|ui| {
                if busy {
                    ui.spinner();
                }
                ui.colored_label(color, status_text);
                if let Some(t0) = step_started.filter(|_| busy) {
                    let elapsed = t0.elapsed();
                    let mut timing = format!("elapsed {}", mm_ss(elapsed.as_secs()));
                    // Linear ETA once enough of the download has arrived to estimate from.
                    if let Some(f) = download.filter(|f| *f > 0.02 && *f < 1.0) {
                        let left = elapsed.as_secs_f32() * (1.0 - f) / f;
                        timing.push_str(&format!(", about {} left", mm_ss(left as u64)));
                    }
                    ui.weak(timing);
                }
            });
            // Model download: a real bar so a multi-GB fetch doesn't look like a hang.
            if let Some(f) = download {
                ui.add(egui::ProgressBar::new(f).desired_width(380.0).show_percentage().animate(true));
            }
            if busy && !hint.is_empty() {
                ui.small(hint);
            }
            ui.add_space(8.0);

            let (label, enabled) = match state {
                UiState::Recording => ("Stop recording (saves and summarizes)", !self.closing),
                UiState::Idle => ("Start recording", true),
                UiState::Loading if download.is_some() => ("Downloading model...", false),
                UiState::Loading => ("Loading model...", false),
                UiState::Summarizing => ("Summarizing...", false),
                _ => ("Please wait...", false),
            };
            let button = egui::Button::new(egui::RichText::new(label).size(18.0)).min_size(egui::vec2(380.0, 48.0));
            if ui.add_enabled(enabled, button).clicked() {
                self.send(Cmd::Toggle);
            }
            ui.small(format!("Global hotkey: {}", self.hotkey_label));
            if cfg.summarize && cfg.chat_provider == ChatProvider::OpenAi && !summarize::is_local_endpoint(&cfg.chat_base_url) {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 40),
                    format!("Summaries are sent to {}", summarize::endpoint_host(&cfg.chat_base_url)),
                );
            }

            ui.separator();
            ui.label("Meeting context (why this meeting is happening; focuses the summary)");
            {
                // The context is shared state: the backend appends dictated text, we edit it here.
                let mut shared = self.ui.shared.lock().unwrap();
                ui.add(
                    egui::TextEdit::multiline(&mut shared.context)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .char_limit(summarize::CONTEXT_MAX_CHARS)
                        .hint_text("e.g. Weekly sync to settle the launch date and unblock the API migration"),
                );
                ui.small(format!(
                    "{}/{} characters, about one paragraph. Plain description only; it is never run as code \
                     and instructions in it are ignored.",
                    shared.context.chars().count(),
                    summarize::CONTEXT_MAX_CHARS
                ));
            }
            ui.horizontal(|ui| {
                let label = if dictating { "Stop dictating" } else { "Dictate context" };
                if ui.add_enabled(idle, egui::Button::new(label)).clicked() {
                    self.send(Cmd::Dictate);
                }
                ui.small(format!("Hotkey: {} (not while recording)", self.dictate_label));
            });

            ui.separator();
            // Usable during the startup it slows down; takes effect from the next launch.
            ui.label("Hardware acceleration (applies on next start)");
            let mut accel = cfg.accelerator.clone();
            egui::ComboBox::from_id_salt("accelerator")
                .width(400.0)
                .selected_text(accel_label(&accel, &eps))
                .show_ui(ui, |ui| {
                    let mut options: Vec<String> =
                        [config::ACCEL_AUTO, config::ACCEL_ALL, config::ACCEL_CPU].map(String::from).into();
                    options.extend(eps.iter().cloned());
                    if !options.contains(&accel) {
                        options.push(accel.clone()); // a hand-edited list from config.toml
                    }
                    for o in options {
                        let label = accel_label(&o, &eps);
                        ui.selectable_value(&mut accel, o, label);
                    }
                });
            if accel != cfg.accelerator {
                self.send(Cmd::SetAccelerator(accel));
            }

            ui.add_space(4.0);
            ui.add_enabled_ui(idle && !dictating, |ui| {
                ui.label("Speech model (live transcription)");
                let a = model_combo(ui, "speech", &speech, &mut self.speech_sel, "nemotron-speech-streaming-en-0.6b");
                ui.add_space(4.0);
                ui.label("Summary provider");
                egui::ComboBox::from_id_salt("provider")
                    .width(400.0)
                    .selected_text(provider_label(self.provider_sel))
                    .show_ui(ui, |ui| {
                        for p in [ChatProvider::Foundry, ChatProvider::OpenAi] {
                            ui.selectable_value(&mut self.provider_sel, p, provider_label(p));
                        }
                    });
                let mut b = false;
                if self.provider_sel == ChatProvider::Foundry {
                    ui.add_space(4.0);
                    ui.label("Chat model (summary)");
                    b = model_combo(ui, "chat", &chat, &mut self.chat_sel, summarize::AUTO);
                    let mut keep = cfg.keep_chat_loaded;
                    ui.checkbox(&mut keep, "Keep chat model loaded (faster summaries, uses more memory)");
                    if keep != cfg.keep_chat_loaded {
                        self.send(Cmd::SetKeepChatLoaded(keep));
                    }
                } else {
                    ui.small("Base URL, e.g. http://localhost:11434/v1 (Ollama), http://localhost:1234/v1 (LM Studio)");
                    ui.add(egui::TextEdit::singleline(&mut self.api_url).desired_width(400.0));
                    ui.small("Model name");
                    ui.add(egui::TextEdit::singleline(&mut self.api_model).desired_width(400.0));
                    ui.small("Environment variable holding the API key (empty for none)");
                    ui.add(egui::TextEdit::singleline(&mut self.api_key_env).desired_width(400.0));
                    ui.small(if self.api_key_saved {
                        "API key: saved (encrypted for this Windows user on this machine)"
                    } else {
                        "API key: none saved (or set the variable above)"
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.api_key_input).password(true).desired_width(280.0),
                        );
                        let can_save = !self.api_key_input.trim().is_empty();
                        if ui.add_enabled(can_save, egui::Button::new("Save key")).clicked() {
                            let key = std::mem::take(&mut self.api_key_input);
                            self.send(Cmd::SetChatApiKey(Some(key)));
                        }
                        if ui.add_enabled(self.api_key_saved, egui::Button::new("Clear key")).clicked() {
                            self.send(Cmd::SetChatApiKey(None));
                        }
                    });
                    if ui.button("Apply").clicked() {
                        self.send(Cmd::SetChatProvider {
                            provider: self.provider_sel,
                            base_url: self.api_url.clone(),
                            api_model: self.api_model.clone(),
                            api_key_env: self.api_key_env.clone(),
                        });
                    }
                    ui.small("The transcript text is sent to this endpoint for the summary. Transcription stays on this machine.");
                }
                if self.provider_sel == ChatProvider::Foundry && cfg.chat_provider != ChatProvider::Foundry {
                    self.send(Cmd::SetChatProvider {
                        provider: ChatProvider::Foundry,
                        base_url: cfg.chat_base_url.clone(),
                        api_model: cfg.chat_api_model.clone(),
                        api_key_env: cfg.chat_api_key_env.clone(),
                    });
                }
                if a || b {
                    self.send(Cmd::ApplyModels { speech: self.speech_sel.clone(), chat: self.chat_sel.clone() });
                }
                ui.small("Models marked \"will download\" are fetched from Foundry Local when selected.");

                ui.add_space(8.0);
                ui.label("Notes folder");
                ui.horizontal(|ui| {
                    let edit = ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(300.0));
                    if edit.lost_focus() && !self.folder.trim().is_empty() {
                        self.send(Cmd::SetNotesDir(PathBuf::from(self.folder.trim())));
                    }
                    if ui.button("Browse...").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().set_directory(&self.folder).pick_folder() {
                            self.folder = dir.to_string_lossy().to_string();
                            self.send(Cmd::SetNotesDir(dir));
                        }
                    }
                    if ui.button("Open").clicked() {
                        let _ = std::fs::create_dir_all(&self.folder);
                        open_in_explorer(Path::new(&self.folder));
                    }
                });
            });

            ui.separator();
            if !partial.is_empty() {
                ui.weak(partial);
            }
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(190.0).show(ui, |ui| {
                for line in &recent {
                    ui.label(line);
                }
            });
            if let Some(p) = last_note {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.small(format!("Note: {}", p.display()));
                    if ui.small_button("Open").clicked() {
                        open_in_explorer(&p);
                    }
                });
            }
        });
    }
}
