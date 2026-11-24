//! Settings window: service status and a test dictation, models, dictionary, settings.
//!
//! The window is a client of the background service. It edits the same plain files the
//! command line does, so both always agree.

use eframe::egui;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::catalog;
use crate::config::{self, Config};
use crate::shortcut::{self, Combo};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dictation,
    Models,
    Dictionary,
    Settings,
}

/// What the polling thread last learned from the service.
#[derive(Default, Clone)]
struct Service {
    running: bool,
    word: String,
    model: String,
    backend: String,
    error: String,
    last: String,
}

#[derive(Default)]
struct FileJob {
    busy: bool,
    done: Option<Result<String, String>>,
}

struct Download {
    file: &'static str,
    bytes: u64,
    part: std::path::PathBuf,
    child: Child,
}

/// Language names for the picker; a code missing here is shown as the code itself.
const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("af", "Afrikaans"), ("am", "Amharic"), ("ar", "Arabic"), ("as", "Assamese"), ("az", "Azerbaijani"),
    ("ba", "Bashkir"), ("be", "Belarusian"), ("bg", "Bulgarian"), ("bn", "Bengali"), ("bo", "Tibetan"),
    ("br", "Breton"), ("bs", "Bosnian"), ("ca", "Catalan"), ("cs", "Czech"), ("cy", "Welsh"),
    ("da", "Danish"), ("de", "German"), ("el", "Greek"), ("en", "English"), ("es", "Spanish"),
    ("et", "Estonian"), ("eu", "Basque"), ("fa", "Persian"), ("fi", "Finnish"), ("fo", "Faroese"),
    ("fr", "French"), ("gl", "Galician"), ("gu", "Gujarati"), ("ha", "Hausa"), ("haw", "Hawaiian"),
    ("he", "Hebrew"), ("hi", "Hindi"), ("hr", "Croatian"), ("ht", "Haitian Creole"), ("hu", "Hungarian"),
    ("hy", "Armenian"), ("id", "Indonesian"), ("is", "Icelandic"), ("it", "Italian"), ("ja", "Japanese"),
    ("jw", "Javanese"), ("ka", "Georgian"), ("kk", "Kazakh"), ("km", "Khmer"), ("kn", "Kannada"),
    ("ko", "Korean"), ("la", "Latin"), ("lb", "Luxembourgish"), ("ln", "Lingala"), ("lo", "Lao"),
    ("lt", "Lithuanian"), ("lv", "Latvian"), ("mg", "Malagasy"), ("mi", "Maori"), ("mk", "Macedonian"),
    ("ml", "Malayalam"), ("mn", "Mongolian"), ("mr", "Marathi"), ("ms", "Malay"), ("mt", "Maltese"),
    ("my", "Burmese"), ("ne", "Nepali"), ("nl", "Dutch"), ("nn", "Norwegian Nynorsk"), ("no", "Norwegian"),
    ("oc", "Occitan"), ("pa", "Punjabi"), ("pl", "Polish"), ("ps", "Pashto"), ("pt", "Portuguese"),
    ("ro", "Romanian"), ("ru", "Russian"), ("sa", "Sanskrit"), ("sd", "Sindhi"), ("si", "Sinhala"),
    ("sk", "Slovak"), ("sl", "Slovenian"), ("sn", "Shona"), ("so", "Somali"), ("sq", "Albanian"),
    ("sr", "Serbian"), ("su", "Sundanese"), ("sv", "Swedish"), ("sw", "Swahili"), ("ta", "Tamil"),
    ("te", "Telugu"), ("tg", "Tajik"), ("th", "Thai"), ("tk", "Turkmen"), ("tl", "Tagalog"),
    ("tr", "Turkish"), ("tt", "Tatar"), ("uk", "Ukrainian"), ("ur", "Urdu"), ("uz", "Uzbek"),
    ("vi", "Vietnamese"), ("yi", "Yiddish"), ("yo", "Yoruba"), ("yue", "Cantonese"), ("zh", "Chinese"),
];

fn language_name(code: &str) -> String {
    LANGUAGE_NAMES
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// The language picker while it is open.
struct LanguageDialog {
    /// Codes offered: what the loaded model reports, else every known language.
    available: Vec<String>,
    selected: Vec<String>,
    search: String,
    from_model: bool,
}

struct App {
    page: Page,
    service: Arc<Mutex<Service>>,
    file_job: Arc<Mutex<FileJob>>,
    config: Config,
    devices: Vec<(String, String)>,
    sources: Vec<String>,
    installed: Vec<String>,
    download: Option<Download>,
    confirm_remove: Option<&'static str>,
    notice: String,
    result: String,
    shown_last: String,
    terms: String,
    terms_note: String,
    corrections: String,
    corrections_note: String,
    autostart: bool,
    shortcut: Option<Combo>,
    shortcut_note: String,
    capturing: bool,
    captured: Vec<egui::Event>,
    lang_dialog: Option<LanguageDialog>,
    detector: Option<Child>,
    shot: Option<(String, u32)>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn installed_files() -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(config::models_dir())
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".gguf"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn capture_sources() -> Vec<String> {
    Command::new("pactl")
        .args(["list", "short", "sources"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split('\t').nth(1).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Models whose upstream catalogue entry lists Slovak among their languages.
fn speaks_slovak(file: &str) -> bool {
    const PARAKEET_V3_SET: &[&str] = &[
        "parakeet-tdt-0.6b-v3-Q4_K_M.gguf",
        "parakeet-tdt-0.6b-v3-Q8_0.gguf",
        "parakeet-primeline-Q4_K_M.gguf",
        "parakeet-ultra-Q4_K_M.gguf",
        "canary-1b-v2-Q4_K_M.gguf",
    ];
    PARAKEET_V3_SET.contains(&file) || (file.starts_with("whisper-") && !file.contains(".en-"))
}

fn poll(service: Arc<Mutex<Service>>, ctx: egui::Context) {
    std::thread::spawn(move || loop {
        let mut next = Service::default();
        if let Ok(reply) = crate::daemon::request("status") {
            next.running = true;
            let (head, error) = match reply.split_once(" error=") {
                Some((head, error)) => (head.to_string(), error.to_string()),
                None => (reply.clone(), String::new()),
            };
            next.error = error;
            for (index, part) in head.split(' ').enumerate() {
                if index == 0 {
                    next.word = part.to_string();
                } else if let Some(value) = part.strip_prefix("model=") {
                    next.model = value.to_string();
                } else if let Some(value) = part.strip_prefix("backend=") {
                    next.backend = value.to_string();
                }
            }
            if next.word == "idle" {
                if let Ok(last) = crate::daemon::request("last") {
                    next.last = last.strip_prefix("ok ").unwrap_or("").to_string();
                }
            } else {
                next.last = lock(&service).last.clone();
            }
        }
        *lock(&service) = next;
        ctx.request_repaint();
        std::thread::sleep(Duration::from_millis(400));
    });
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
            style.spacing.interact_size.y = 30.0;
            for (text_style, font) in style.text_styles.iter_mut() {
                font.size = match text_style {
                    egui::TextStyle::Heading => 22.0,
                    egui::TextStyle::Small => 12.0,
                    _ => 15.0,
                };
            }
        });
        // The stock theme draws secondary text and control outlines too faintly to read.
        for (theme, weak, outline) in [
            (egui::Theme::Dark, egui::Color32::from_gray(175), egui::Color32::from_gray(110)),
            (egui::Theme::Light, egui::Color32::from_gray(80), egui::Color32::from_gray(150)),
        ] {
            cc.egui_ctx.style_mut_of(theme, |style| {
                style.visuals.weak_text_color = Some(weak);
                style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, outline);
            });
        }
        let service = Arc::new(Mutex::new(Service::default()));
        poll(service.clone(), cc.egui_ctx.clone());
        let config = Config::load();
        let devices = transcribe_cpp::devices()
            .into_iter()
            .map(|device| (device.name, device.description))
            .collect();
        let shot = std::env::var("OSTT_GUI_SHOT").ok().map(|path| (path, 0));
        let page = match std::env::var("OSTT_GUI_PAGE").as_deref() {
            Ok("models") => Page::Models,
            Ok("dictionary") => Page::Dictionary,
            Ok("settings") => Page::Settings,
            _ => Page::Dictation,
        };
        App {
            page,
            service,
            file_job: Arc::new(Mutex::new(FileJob::default())),
            shortcut: shortcut::current(),
            shortcut_note: String::new(),
            capturing: std::env::var_os("OSTT_GUI_CAPTURE").is_some(),
            captured: Vec::new(),
            lang_dialog: None,
            detector: None,
            config,
            devices,
            sources: capture_sources(),
            installed: installed_files(),
            download: None,
            confirm_remove: None,
            notice: String::new(),
            result: String::new(),
            shown_last: String::new(),
            terms: std::fs::read_to_string(config::terms_path()).unwrap_or_default(),
            terms_note: String::new(),
            corrections: std::fs::read_to_string(config::corrections_path()).unwrap_or_default(),
            corrections_note: String::new(),
            autostart: systemctl(&["is-enabled", "--quiet", "ostt.service"]),
            shot,
        }
    }

    /// Writes one setting; engine settings also make the service reload its model.
    fn set(&mut self, key: &str, value: &str) {
        match Config::set(key, value) {
            Ok(()) => {
                self.config = Config::load();
                if matches!(key, "model" | "device" | "language" | "languages" | "translate" | "threads") {
                    let _ = crate::daemon::request("reload");
                }
                self.notice.clear();
            }
            Err(error) => self.notice = error,
        }
    }

    fn active_file(&self) -> String {
        self.config
            .model_path()
            .ok()
            .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    fn needs_language(&self) -> bool {
        self.active_file().starts_with("canary") && self.config.get("language").eq_ignore_ascii_case("auto")
            && self.config.get("languages").is_empty()
    }

    fn status_line(&self, ui: &mut egui::Ui, service: &Service) {
        let (color, label) = if !service.running {
            (egui::Color32::from_rgb(150, 150, 150), "Service not running".to_string())
        } else {
            match service.word.as_str() {
                "recording" => (egui::Color32::from_rgb(220, 60, 60), "Recording".to_string()),
                "transcribing" => (egui::Color32::from_rgb(230, 160, 40), "Transcribing".to_string()),
                "loading" => (egui::Color32::from_rgb(230, 160, 40), "Loading model".to_string()),
                "error" => (egui::Color32::from_rgb(220, 60, 60), "Model not loaded".to_string()),
                _ => (egui::Color32::from_rgb(70, 170, 90), "Ready".to_string()),
            }
        };
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 6.0, color);
            ui.label(egui::RichText::new(label).heading().strong());
        });
        if service.running && !service.model.is_empty() {
            ui.label(egui::RichText::new(format!("{}  ·  {}", service.model, service.backend)).weak());
        }
        if !service.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(220, 60, 60), &service.error);
        }
    }

    fn page_dictation(&mut self, ui: &mut egui::Ui, service: &Service) {
        self.status_line(ui, service);
        ui.add_space(6.0);
        if !service.running {
            ui.label("The background service keeps the model loaded and listens for the shortcut.");
            if ui.button("Start service").clicked() && !systemctl(&["start", "ostt.service"]) {
                self.notice = "Could not start ostt.service — run ./install.sh first.".into();
            }
        }
        if self.needs_language() {
            ui.colored_label(
                egui::Color32::from_rgb(230, 160, 40),
                "Canary needs a language. Without one it may translate into English — set it under Settings.",
            );
        }

        ui.separator();
        ui.label(egui::RichText::new("Try it").strong());
        ui.label(egui::RichText::new("A test recording shows the text here and types nothing.").weak());
        let file_busy = lock(&self.file_job).busy;
        ui.horizontal(|ui| {
            let recording = service.word == "recording";
            let ready = service.running && (service.word == "idle" || recording) && !file_busy;
            let label = if recording { "Finish and transcribe" } else { "Start test recording" };
            if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                let _ = crate::daemon::request(if recording { "stop" } else { "start-test" });
            }
            if recording && ui.button("Discard").clicked() {
                let _ = crate::daemon::request("cancel");
            }
            let idle = service.running && service.word == "idle" && !file_busy;
            if ui.add_enabled(idle, egui::Button::new("Transcribe a file…")).clicked() {
                self.pick_file();
            }
            if file_busy {
                ui.spinner();
                ui.label("Transcribing file…");
            }
        });
        if let Some(done) = lock(&self.file_job).done.take() {
            match done {
                Ok(text) => self.result = text,
                Err(error) => self.notice = error,
            }
        }
        if service.last != self.shown_last {
            self.shown_last = service.last.clone();
            self.result = service.last.clone();
        }
        let mut shown = self.result.clone();
        ui.add(
            egui::TextEdit::multiline(&mut shown)
                .desired_rows(9)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(10, 8))
                .hint_text("The transcript appears here."),
        );
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.result.is_empty(), egui::Button::new("Copy")).clicked() {
                ui.ctx().copy_text(self.result.clone());
            }
            if ui.add_enabled(!self.result.is_empty(), egui::Button::new("Clear")).clicked() {
                self.result.clear();
            }
        });

        ui.separator();
        ui.label(egui::RichText::new("Keyboard shortcut").strong());
        ui.horizontal(|ui| {
            match &self.shortcut {
                Some(combo) => {
                    ui.monospace(combo.to_string());
                    ui.label(egui::RichText::new("starts and finishes dictation.").weak());
                }
                None => {
                    ui.label(egui::RichText::new("None set; dictation starts from the tray icon.").weak());
                }
            }
            if ui.small_button("Change…").clicked() {
                self.page = Page::Settings;
            }
        });
        ui.add_space(4.0);
        if ui.checkbox(&mut self.autostart, "Start the service when I log in").changed() {
            let wanted = self.autostart;
            if !systemctl(&[if wanted { "enable" } else { "disable" }, "ostt.service"]) {
                self.autostart = !wanted;
                self.notice = "Could not change the login setting — run ./install.sh first.".into();
            }
        }
    }

    fn pick_file(&mut self) {
        let job = self.file_job.clone();
        lock(&job).busy = true;
        std::thread::spawn(move || {
            let picked = Command::new("zenity")
                .args(["--file-selection", "--title=Choose an audio file"])
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .filter(|path| !path.is_empty());
            let done = picked.map(|path| match crate::daemon::request(&format!("file {path}")) {
                Ok(reply) => match reply.strip_prefix("ok") {
                    Some(text) => Ok(text.trim().to_string()),
                    None => Err(reply.strip_prefix("error ").unwrap_or(&reply).to_string()),
                },
                Err(error) => Err(error),
            });
            let mut job = lock(&job);
            job.busy = false;
            job.done = done;
        });
    }

    fn page_models(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Speech models").heading().strong());
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Downloads come from huggingface.co/handy-computer and are installed only when size and SHA-256 match.")
                .weak(),
        );
        if self.needs_language() {
            ui.colored_label(
                egui::Color32::from_rgb(230, 160, 40),
                "Canary needs a language. Without one it may translate into English — set it under Settings.",
            );
        }

        // A finished download: refresh the list and report a failure.
        let mut finished = None;
        if let Some(download) = self.download.as_mut() {
            if let Ok(Some(status)) = download.child.try_wait() {
                finished = Some((download.file, status.success()));
            }
        }
        if let Some((file, ok)) = finished {
            self.download = None;
            self.installed = installed_files();
            self.notice = if ok { String::new() } else { format!("Download of {file} failed; press Download again to resume.") };
        }

        let active = self.active_file();
        let downloading = self.download.as_ref().map(|download| download.file);
        let mut action: Option<(&'static str, &'static str)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut family = "";
            for entry in catalog::MODELS {
                if entry.family != family {
                    family = entry.family;
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(family).strong());
                }
                let installed = self.installed.iter().any(|name| name == entry.file);
                let is_active = active == entry.file;
                ui.horizontal(|ui| {
                    ui.set_min_height(30.0);
                    let name = entry.file.trim_end_matches(".gguf");
                    let text = if is_active { egui::RichText::new(name).strong() } else { egui::RichText::new(name) };
                    ui.add_sized([330.0, 20.0], egui::Label::new(text).halign(egui::Align::LEFT));
                    ui.add_sized(
                        [70.0, 20.0],
                        egui::Label::new(egui::RichText::new(format!("{:.0} MB", entry.bytes as f64 / 1e6)).weak()),
                    );
                    ui.add_sized(
                        [30.0, 20.0],
                        egui::Label::new(egui::RichText::new(if speaks_slovak(entry.file) { "SK" } else { "" }).weak()),
                    );
                    if is_active {
                        ui.label(egui::RichText::new("✔ In use").color(egui::Color32::from_rgb(70, 170, 90)));
                    } else if downloading == Some(entry.file) {
                        let done = self
                            .download
                            .as_ref()
                            .and_then(|download| std::fs::metadata(&download.part).ok().map(|m| m.len() as f32 / download.bytes as f32))
                            .unwrap_or(0.0);
                        ui.add(egui::ProgressBar::new(done.min(1.0)).desired_width(150.0).show_percentage());
                        if ui.button("Cancel").clicked() {
                            action = Some(("cancel", entry.file));
                        }
                    } else if installed {
                        if ui.button("Use").clicked() {
                            action = Some(("use", entry.file));
                        }
                        if self.confirm_remove == Some(entry.file) {
                            if ui.button("Really remove").clicked() {
                                action = Some(("remove", entry.file));
                            }
                            if ui.button("Keep").clicked() {
                                action = Some(("keep", entry.file));
                            }
                        } else if ui.button("Remove").clicked() {
                            action = Some(("ask", entry.file));
                        }
                    } else if ui.add_enabled(downloading.is_none(), egui::Button::new("Download")).clicked() {
                        action = Some(("download", entry.file));
                    }
                });
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new("SK = the catalogue lists Slovak for this model.").weak());
        });

        match action {
            Some(("use", file)) => self.set("model", file),
            Some(("ask", file)) => self.confirm_remove = Some(file),
            Some(("keep", _)) => self.confirm_remove = None,
            Some(("remove", file)) => {
                let _ = std::fs::remove_file(config::models_dir().join(file));
                self.confirm_remove = None;
                self.installed = installed_files();
            }
            Some(("cancel", _)) => {
                if let Some(mut download) = self.download.take() {
                    let _ = download.child.kill();
                    let _ = download.child.wait();
                }
            }
            Some(("download", file)) => {
                let entry = catalog::MODELS.iter().find(|entry| entry.file == file);
                let exe = std::env::current_exe();
                if let (Some(entry), Ok(exe)) = (entry, exe) {
                    match Command::new(exe)
                        .args(["download", &entry.id()])
                        .env("OSTT_QUIET_DOWNLOAD", "1")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(child) => {
                            self.download = Some(Download {
                                file: entry.file,
                                bytes: entry.bytes,
                                part: config::models_dir().join(format!("{}.part", entry.file)),
                                child,
                            })
                        }
                        Err(error) => self.notice = error.to_string(),
                    }
                }
            }
            _ => {}
        }
    }

    fn page_dictionary(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Dictionary").heading().strong());
        ui.add_space(4.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(egui::RichText::new("Preferred words").strong());
            ui.label(
                egui::RichText::new("One word or phrase per line. A close spelling in the transcript is replaced by yours. Up to 128 entries.")
                    .weak(),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.terms)
                    .desired_rows(7)
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 8))
                    .hint_text("Tailscale"),
            );
            ui.horizontal(|ui| {
                if ui.button("Save words").clicked() {
                    self.terms_note = save_checked(&config::terms_path(), &self.terms, |path, raw| {
                        raw.trim().is_empty() || crate::custom_terms::load_terms(path).is_some()
                    });
                }
                ui.label(egui::RichText::new(&self.terms_note).weak());
            });

            ui.add_space(12.0);
            ui.label(egui::RichText::new("Exact corrections").strong());
            ui.label(
                egui::RichText::new("One rule per line: what was heard => what to write. Up to 32 rules; digits must stay the same on both sides.")
                    .weak(),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.corrections)
                    .desired_rows(7)
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 8))
                    .hint_text("tail scale => Tailscale"),
            );
            ui.horizontal(|ui| {
                if ui.button("Save corrections").clicked() {
                    self.corrections_note = save_checked(&config::corrections_path(), &self.corrections, |_, raw| {
                        raw.trim().is_empty() || crate::personal_corrections::parse(raw).is_ok()
                    });
                }
                ui.label(egui::RichText::new(&self.corrections_note).weak());
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Both lists apply from the next dictation; no restart needed.").weak());
        });
    }

    fn page_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Settings").heading().strong());
        ui.add_space(4.0);
        let mut change: Option<(&'static str, String)> = None;
        let (mut toggle_capture, mut remove_shortcut, mut open_languages) = (false, false, false);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([24.0, 12.0]).show(ui, |ui| {
                ui.label("Dictation shortcut");
                ui.horizontal(|ui| {
                    let text = if self.capturing {
                        "Press the shortcut…   (Esc cancels)".to_string()
                    } else {
                        self.shortcut.as_ref().map(|combo| combo.to_string()).unwrap_or_else(|| "None — click to set".into())
                    };
                    let field = egui::Button::new(text).selected(self.capturing).min_size(egui::vec2(260.0, 30.0));
                    if ui.add_enabled(shortcut::supported(), field).clicked() {
                        toggle_capture = true;
                    }
                    let removable = self.shortcut.is_some() && !self.capturing;
                    if ui.add_enabled(removable, egui::Button::new("Remove")).clicked() {
                        remove_shortcut = true;
                    }
                });
                ui.end_row();

                ui.label("");
                let note = if !self.shortcut_note.is_empty() {
                    self.shortcut_note.clone()
                } else if !shortcut::supported() {
                    "On this desktop, bind `ostt toggle` in the keyboard settings.".into()
                } else if self.shortcut.is_none() {
                    "Without a shortcut, dictation starts from the tray icon.".into()
                } else {
                    "Press it once to start dictating and again to finish.".into()
                };
                ui.vertical(|ui| {
                    ui.set_max_width(520.0);
                    ui.add(egui::Label::new(egui::RichText::new(note).weak()).wrap());
                });
                ui.end_row();

                ui.label("Language");
                ui.horizontal(|ui| {
                    let chosen = self.chosen_languages();
                    let summary = if chosen.is_empty() {
                        "Automatic".to_string()
                    } else {
                        chosen.iter().map(|code| language_name(code)).collect::<Vec<_>>().join(", ")
                    };
                    if ui.add(egui::Button::new(summary).min_size(egui::vec2(260.0, 30.0))).clicked() {
                        open_languages = true;
                    }
                    ui.label(egui::RichText::new("click to choose").weak());
                });
                ui.end_row();

                ui.label("Compute device");
                let device = self.config.get("device");
                let shown = self
                    .devices
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&device))
                    .map(|(name, description)| format!("{name} — {description}"))
                    .unwrap_or_else(|| "Automatic (integrated GPU first)".into());
                egui::ComboBox::from_id_salt("device").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(device == "auto", "Automatic (integrated GPU first)").clicked() {
                        change = Some(("device", "auto".into()));
                    }
                    for (name, description) in &self.devices {
                        if ui.selectable_label(device.eq_ignore_ascii_case(name), format!("{name} — {description}")).clicked() {
                            change = Some(("device", name.clone()));
                        }
                    }
                });
                ui.end_row();

                ui.label("Microphone");
                let source = self.config.get("source");
                let shown = if source.is_empty() { "System default".to_string() } else { source.clone() };
                egui::ComboBox::from_id_salt("source").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(source.is_empty(), "System default").clicked() {
                        change = Some(("source", String::new()));
                    }
                    for name in &self.sources {
                        if ui.selectable_label(&source == name, name).clicked() {
                            change = Some(("source", name.clone()));
                        }
                    }
                });
                ui.end_row();

                ui.label("Insert text by");
                let inject = self.config.get("inject");
                let options = [
                    ("type", "Typing into the focused window"),
                    ("paste", "Clipboard, then Ctrl+V"),
                    ("copy", "Clipboard only"),
                ];
                let shown = options.iter().find(|(key, _)| *key == inject).map(|(_, label)| *label).unwrap_or("Typing into the focused window");
                egui::ComboBox::from_id_salt("inject").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                    for (key, label) in options {
                        if ui.selectable_label(inject == key, label).clicked() {
                            change = Some(("inject", key.into()));
                        }
                    }
                });
                ui.end_row();

                ui.label("Stop after silence");
                let auto_stop = self.config.number("auto_stop_ms");
                let options = [(0u64, "Off — I press the shortcut again"), (2000, "2 seconds"), (3000, "3 seconds"), (5000, "5 seconds")];
                let shown = options.iter().find(|(value, _)| *value == auto_stop).map(|(_, label)| *label).unwrap_or("2 seconds");
                egui::ComboBox::from_id_salt("auto_stop").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                    for (value, label) in options {
                        if ui.selectable_label(auto_stop == value, label).clicked() {
                            change = Some(("auto_stop_ms", value.to_string()));
                        }
                    }
                });
                ui.end_row();

                ui.label("Notifications");
                let notify = self.config.get("notify");
                let options = [("all", "Recording, result and errors"), ("errors", "Errors only"), ("off", "None")];
                let shown = options.iter().find(|(key, _)| *key == notify).map(|(_, label)| *label).unwrap_or("Recording, result and errors");
                egui::ComboBox::from_id_salt("notify").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                    for (key, label) in options {
                        if ui.selectable_label(notify == key, label).clicked() {
                            change = Some(("notify", key.into()));
                        }
                    }
                });
                ui.end_row();

                ui.label("");
                let mut append = self.config.flag("append_space");
                if ui.checkbox(&mut append, "Add a space after inserted text").changed() {
                    change = Some(("append_space", append.to_string()));
                }
                ui.end_row();

                ui.label("");
                let mut translate = self.config.flag("translate");
                if ui.checkbox(&mut translate, "Translate speech to English (Whisper models)").changed() {
                    change = Some(("translate", translate.to_string()));
                }
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("Settings file: {}", config::config_path().display())).weak());
        });
        if let Some((key, value)) = change {
            self.set(key, &value);
        }
        if toggle_capture {
            self.capturing = !self.capturing;
            self.shortcut_note.clear();
        }
        if remove_shortcut {
            self.shortcut_note = match shortcut::clear() {
                Ok(()) => String::new(),
                Err(error) => error,
            };
            self.shortcut = shortcut::current();
        }
        if open_languages {
            self.open_language_dialog();
        }
    }

    /// The languages in effect: the list when it is set, else the single hint, else none.
    fn chosen_languages(&self) -> Vec<String> {
        let list = crate::language_selection::parse_allowed(&self.config.get("languages"));
        if !list.is_empty() {
            return list;
        }
        crate::language_selection::parse_allowed(&self.config.get("language"))
            .into_iter()
            .filter(|code| code != "auto")
            .collect()
    }

    fn open_language_dialog(&mut self) {
        let selected = self.chosen_languages();
        let reported: Vec<String> = crate::daemon::request("languages")
            .ok()
            .and_then(|reply| reply.strip_prefix("ok").map(|list| list.trim().to_string()))
            .map(|list| list.split(',').filter(|code| !code.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        let from_model = !reported.is_empty();
        let mut available = if from_model {
            reported
        } else {
            LANGUAGE_NAMES.iter().map(|(code, _)| code.to_string()).collect()
        };
        for code in &selected {
            if !available.contains(code) {
                available.push(code.clone());
            }
        }
        // Ticked languages first, then alphabetically by name.
        available.sort_by_key(|code| (!selected.contains(code), language_name(code)));
        self.lang_dialog = Some(LanguageDialog { available, selected, search: String::new(), from_model });
    }

    /// One key press while the shortcut field is listening.
    fn capture(&mut self, event: &egui::Event) {
        // The toolkit turns Ctrl+C, Ctrl+X and Ctrl+V into clipboard events before they reach
        // us as keys; they are still shortcut attempts and must get their refusal.
        let clipboard = |key: &str| Combo { ctrl: true, key: key.into(), ..Default::default() };
        let attempt = match event {
            egui::Event::Copy => Some(clipboard("c")),
            egui::Event::Cut => Some(clipboard("x")),
            egui::Event::Paste(_) => Some(clipboard("v")),
            _ => None,
        };
        if let Some(combo) = attempt {
            if let Err(reason) = shortcut::check(&combo, &shortcut::taken()) {
                self.shortcut_note = reason;
            }
            return;
        }
        let egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event else { return };
        // A modifier on its way down is not the shortcut yet; wait for the key that follows.
        let name = key.name();
        if ["Shift", "Control", "Alt", "Super", "Meta"].iter().any(|prefix| name.starts_with(prefix)) {
            return;
        }
        if *key == egui::Key::Escape && modifiers.is_none() {
            self.capturing = false;
            self.shortcut_note.clear();
            return;
        }
        let Some(name) = shortcut::xkb_key(key.name()) else {
            self.shortcut_note = format!("{} cannot be used in a shortcut. Try another key.", key.name());
            return;
        };
        let combo = Combo { superkey: modifiers.mac_cmd, ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift, key: name };
        match shortcut::set(&combo) {
            Ok(()) => {
                self.capturing = false;
                self.shortcut = shortcut::current();
                self.shortcut_note = format!("{combo} now starts and finishes dictation.");
            }
            // Stay listening, so another combination can be pressed straight away.
            Err(reason) => self.shortcut_note = reason,
        }
    }

    fn language_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.lang_dialog.take() else { return };
        if let Some(child) = self.detector.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                self.detector = None;
            }
        }
        let active = self.active_file();
        let ignores_list = active.starts_with("parakeet");
        let detector_installed = config::langid_path().is_file();
        let (mut apply, mut close) = (false, false);
        let response = egui::Modal::new(egui::Id::new("languages")).show(ctx, |ui| {
            ui.set_width(430.0);
            ui.label(egui::RichText::new("Languages").heading().strong());
            let scope = if dialog.from_model { "Languages the loaded model supports." } else { "All known languages; the model may not support each one." };
            ui.label(egui::RichText::new(format!("Tick the languages you dictate in. {scope}")).weak());
            ui.add(egui::TextEdit::singleline(&mut dialog.search).hint_text("Search").desired_width(f32::INFINITY));
            let needle = dialog.search.trim().to_lowercase();
            egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, false]).show(ui, |ui| {
                let mut shown = 0;
                for code in &dialog.available {
                    let name = language_name(code);
                    if !needle.is_empty() && !name.to_lowercase().contains(&needle) && !code.contains(&needle) {
                        continue;
                    }
                    shown += 1;
                    let mut ticked = dialog.selected.contains(code);
                    let full = !ticked && dialog.selected.len() >= crate::language_selection::MAX_ALLOWED;
                    let label = if name == *code { name } else { format!("{name}   ({code})") };
                    if ui.add_enabled(!full, egui::Checkbox::new(&mut ticked, label)).changed() {
                        if ticked {
                            dialog.selected.push(code.clone());
                        } else {
                            dialog.selected.retain(|other| other != code);
                        }
                    }
                }
                if shown == 0 {
                    ui.label(egui::RichText::new("No language matches the search.").weak());
                }
            });
            ui.separator();
            let names = dialog.selected.iter().map(|code| language_name(code)).collect::<Vec<_>>().join(", ");
            let summary = match dialog.selected.len() {
                0 => "Nothing ticked: the model decides the language itself.".to_string(),
                1 => format!("Recognition is fixed to {names}."),
                _ => format!("Each recording is matched to one of: {names}."),
            };
            ui.label(summary);
            let amber = egui::Color32::from_rgb(230, 160, 40);
            let mut blocked = false;
            if dialog.selected.is_empty() && active.starts_with("canary") {
                ui.colored_label(amber, "Canary needs a language; without one it may translate into English.");
            }
            if dialog.selected.len() >= 2 {
                if ignores_list {
                    ui.colored_label(amber, "Parakeet picks the language itself and ignores this list.");
                } else if !detector_installed {
                    blocked = true;
                    ui.colored_label(amber, "Several languages need the language detector (27 MB, one-time download).");
                    if self.detector.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Downloading the detector…");
                        });
                    } else if ui.button("Download detector").clicked() {
                        if let Ok(exe) = std::env::current_exe() {
                            self.detector = Command::new(exe)
                                .args(["download", "langid"])
                                .env("OSTT_QUIET_DOWNLOAD", "1")
                                .stdin(Stdio::null())
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .spawn()
                                .ok();
                        }
                    }
                }
            }
            if dialog.selected.len() >= crate::language_selection::MAX_ALLOWED {
                ui.label(egui::RichText::new("Eight languages is the limit.").weak());
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!blocked, egui::Button::new("Apply")).clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui.add_enabled(!dialog.selected.is_empty(), egui::Button::new("Untick all")).clicked() {
                    dialog.selected.clear();
                }
            });
        });
        if apply {
            let (language, languages) = match dialog.selected.len() {
                0 => ("auto".to_string(), String::new()),
                1 => (dialog.selected[0].clone(), String::new()),
                _ => ("auto".to_string(), dialog.selected.join(",")),
            };
            let saved = Config::set("language", &language).and_then(|_| Config::set("languages", &languages));
            match saved {
                Ok(()) => {
                    self.config = Config::load();
                    let _ = crate::daemon::request("reload");
                }
                Err(error) => self.notice = error,
            }
        } else if !close && !response.should_close() {
            self.lang_dialog = Some(dialog);
        }
        if self.detector.is_some() {
            ctx.request_repaint_after(Duration::from_millis(300));
        }
    }
}

/// Writes `text` to `path` and reports whether the engine will accept it. A rejected list is
/// still saved, so nothing typed is lost; the note says it is not in effect.
fn save_checked(path: &std::path::Path, text: &str, valid: impl Fn(&std::path::Path, &str) -> bool) -> String {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path, text) {
        Err(error) => format!("Could not save: {error}"),
        Ok(()) if valid(path, text) => "Saved.".into(),
        Ok(()) => "Saved, but not in effect: a line is too long, repeated or malformed.".into(),
    }
}

impl eframe::App for App {
    /// While the shortcut field listens, key presses go to it and to nothing else, so Tab,
    /// Space or Enter cannot operate the window behind it.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !self.capturing {
            return;
        }
        let (keys, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut raw_input.events)
            .into_iter()
            .partition(|event| {
                matches!(
                    event,
                    egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                )
            });
        raw_input.events = rest;
        self.captured.extend(keys);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for event in std::mem::take(&mut self.captured) {
            if self.capturing {
                self.capture(&event);
            }
        }
        if self.capturing && self.page != Page::Settings {
            self.capturing = false;
        }
        if self.shot.is_some() && std::env::var_os("OSTT_GUI_LANGS").is_some() && self.lang_dialog.is_none() {
            self.open_language_dialog();
        }
        let service = lock(&self.service).clone();
        egui::Panel::left("nav").resizable(false).exact_size(170.0).show(ui, |ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("OSTT").heading().strong());
        ui.add_space(4.0);
            ui.label(egui::RichText::new("Offline speech to text").weak().small());
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                for (page, label) in [
                    (Page::Dictation, "Dictation"),
                    (Page::Models, "Models"),
                    (Page::Dictionary, "Dictionary"),
                    (Page::Settings, "Settings"),
                ] {
                    let selected = self.page == page;
                    if ui.add(egui::Button::selectable(selected, label).min_size(egui::vec2(0.0, 32.0))).clicked() {
                        self.page = page;
                        self.notice.clear();
                    }
                }
            });
        });
        if !self.notice.is_empty() {
            egui::Panel::bottom("notice").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(220, 60, 60), &self.notice);
                    if ui.small_button("Dismiss").clicked() {
                        self.notice.clear();
                    }
                });
            });
        }
        let frame = egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::symmetric(22, 14));
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            match self.page {
                Page::Dictation => self.page_dictation(ui, &service),
                Page::Models => self.page_models(ui),
                Page::Dictionary => self.page_dictionary(ui),
                Page::Settings => self.page_settings(ui),
            }
        });
        self.language_dialog(ui.ctx());
        if self.download.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(300));
        }

        // Self-check hook: OSTT_GUI_SHOT=<file.png> renders a few frames, saves one and exits.
        if let Some((path, frames)) = self.shot.as_mut() {
            *frames += 1;
            // With OSTT_GUI_SHOT_WAIT the picture is taken once `<file>.go` appears, and
            // `<file>.focus` says whether the window has keyboard focus.
            let waiting = std::env::var_os("OSTT_GUI_SHOT_WAIT").is_some();
            if waiting {
                let focused = ui.input(|input| input.focused);
                let _ = std::fs::write(format!("{path}.focus"), if focused { "1" } else { "0" });
                if *frames < 1_000_000 && std::path::Path::new(&format!("{path}.go")).exists() {
                    *frames = 1_000_000;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            } else if *frames == 20 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let image = ui.input(|input| {
                input.raw.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(image) = image {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect();
                let saved = std::fs::File::create(&*path).ok().and_then(|file| {
                    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.size[0] as u32, image.size[1] as u32);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.write_header().ok()?.write_image_data(&bytes).ok()
                });
                if saved.is_none() {
                    eprintln!("ostt: could not write {path}");
                }
                std::process::exit(0);
            }
            ui.ctx().request_repaint();
        }
    }
}

/// Opens the settings window as its own process (used by the tray menu).
pub fn spawn_window() {
    if let Ok(exe) = std::env::current_exe() {
        let child = Command::new(exe).arg("gui").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            // Reap it when it closes, so the service does not collect dead children.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

pub fn run() -> Result<(), String> {
    crate::inject::ensure_session_env();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Offline speech to text")
            .with_app_id("ostt")
            .with_inner_size([940.0, 660.0])
            .with_min_inner_size([780.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("ostt", options, Box::new(|cc| Ok(Box::new(App::new(cc))))).map_err(|e| e.to_string())
}
