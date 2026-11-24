//! The background service: keeps the model loaded, records on request and inserts the result.
//!
//! One line per request on a Unix socket in XDG_RUNTIME_DIR; one line back.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio_io::{Recorder, SAMPLE_RATE};
use crate::config::Config;
use crate::engine::Engine;
use crate::interactive_endpointing as endpointing;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Idle,
    Recording,
    Transcribing,
}

struct State {
    phase: Phase,
    loading: bool,
    error: Option<String>,
    recorder: Option<Recorder>,
    /// Bumped whenever a recording starts or ends, so a stale auto-stop watcher does nothing.
    session: u64,
    started: Option<Instant>,
    /// A test recording from the settings window: keep the text, type it nowhere.
    quiet: bool,
    last_text: String,
    model: String,
    backend: String,
    languages: Vec<String>,
}

pub struct Daemon {
    state: Mutex<State>,
    /// Held for the whole model load, so a dictation started meanwhile simply waits for it.
    engine: Mutex<Result<Engine, String>>,
    tray: Mutex<Option<ksni::blocking::Handle<crate::tray::OsttTray>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn log(message: &str) {
    eprintln!("ostt: {message}");
}

impl Daemon {
    fn status_word(&self) -> &'static str {
        let state = lock(&self.state);
        match state.phase {
            Phase::Recording => "recording",
            Phase::Transcribing => "transcribing",
            Phase::Idle if state.loading => "loading",
            Phase::Idle if state.error.is_some() => "error",
            Phase::Idle => "idle",
        }
    }

    fn refresh_tray(&self) {
        let word = self.status_word();
        let detail = {
            let state = lock(&self.state);
            match (&state.error, state.model.is_empty()) {
                (Some(error), _) => error.clone(),
                (None, false) => format!("{} · {}", state.model, state.backend),
                (None, true) => String::new(),
            }
        };
        if let Some(handle) = lock(&self.tray).as_ref() {
            handle.update(move |tray| {
                tray.status = word;
                tray.detail = detail;
            });
        }
    }

    fn notify(&self, title: &str, body: &str, timeout_ms: u32, is_error: bool) {
        let level = Config::load().get("notify");
        if level == "off" || (level == "errors" && !is_error) {
            return;
        }
        crate::inject::notify(title, body, timeout_ms, false);
    }

    pub fn load_engine(self: &Arc<Self>) {
        let daemon = self.clone();
        {
            let mut state = lock(&daemon.state);
            state.loading = true;
            state.error = None;
        }
        daemon.refresh_tray();
        std::thread::spawn(move || {
            let mut slot = lock(&daemon.engine);
            *slot = Err("model not loaded".into());
            let started = Instant::now();
            let loaded = Config::load().engine_options().and_then(|options| Engine::load(&options));
            let mut state = lock(&daemon.state);
            state.loading = false;
            match &loaded {
                Ok(engine) => {
                    state.model = engine
                        .model_path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    state.backend = engine.backend.clone();
                    state.languages = engine.languages.clone();
                    state.error = None;
                    log(&format!(
                        "loaded {} ({}) on {} with {} threads in {:.1}s{}",
                        state.model,
                        engine.arch,
                        engine.backend,
                        engine.threads,
                        started.elapsed().as_secs_f64(),
                        if engine.note.is_empty() { String::new() } else { format!(" — note: {}", engine.note) }
                    ));
                }
                Err(error) => {
                    state.error = Some(error.clone());
                    log(&format!("model load failed: {error}"));
                }
            }
            drop(state);
            if let Err(error) = &loaded {
                crate::inject::notify("OSTT: model not loaded", error, 8000, true);
            }
            *slot = loaded;
            drop(slot);
            daemon.refresh_tray();
        });
    }

    pub fn start(self: &Arc<Self>) -> String {
        self.begin(false)
    }

    fn begin(self: &Arc<Self>, quiet: bool) -> String {
        let config = Config::load();
        let session = {
            let mut state = lock(&self.state);
            match state.phase {
                Phase::Recording => return "ok recording".into(),
                Phase::Transcribing => return "busy transcribing".into(),
                Phase::Idle => {}
            }
            if let (Some(error), false) = (&state.error, state.loading) {
                return format!("error {error}");
            }
            match Recorder::start(&config.get("source")) {
                Ok(recorder) => state.recorder = Some(recorder),
                Err(error) => return format!("error {error}"),
            }
            state.phase = Phase::Recording;
            state.quiet = quiet;
            state.session += 1;
            state.started = Some(Instant::now());
            state.session
        };
        self.refresh_tray();
        if !quiet {
            self.notify("● Recording", "Press the shortcut again to finish.", 600_000, false);
        }
        self.watch(session, config.number("auto_stop_ms") as i32);
        "ok recording".into()
    }

    /// Ends the recording by itself: after a pause when auto-stop is on, when the microphone
    /// stream dies, and always at the ten-minute safety cap.
    fn watch(self: &Arc<Self>, session: u64, auto_stop_ms: i32) {
        let daemon = self.clone();
        let auto_stop = auto_stop_ms > 0;
        let pause_ms = endpointing::resolve_silence_ms(auto_stop_ms);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(100));
            let finish = {
                let mut state = lock(&daemon.state);
                if state.session != session || state.phase != Phase::Recording {
                    return;
                }
                let total = state.started.map(|at| at.elapsed()).unwrap_or_default();
                let Some(recorder) = state.recorder.as_mut() else { return };
                let exited = recorder.exited();
                let capture = lock(&recorder.capture);
                let samples = capture.samples.len();
                let to_duration = |count: usize| Duration::from_millis((count * 1000 / SAMPLE_RATE) as u64);
                exited
                    || endpointing::should_finish_interactive_session(
                        auto_stop,
                        false,
                        samples > 0,
                        capture.speech_started,
                        to_duration(samples.saturating_sub(capture.last_voiced)),
                        to_duration(samples),
                        total,
                        pause_ms,
                    )
            };
            if finish {
                daemon.stop();
                return;
            }
        });
    }

    pub fn stop(self: &Arc<Self>) -> String {
        let recorder = {
            let mut state = lock(&self.state);
            if state.phase != Phase::Recording {
                return format!("ok {}", match state.phase {
                    Phase::Transcribing => "transcribing",
                    _ => "idle",
                });
            }
            state.phase = Phase::Transcribing;
            state.session += 1;
            state.recorder.take()
        };
        self.refresh_tray();
        let daemon = self.clone();
        std::thread::spawn(move || {
            let outcome = daemon.finish(recorder);
            lock(&daemon.state).phase = Phase::Idle;
            daemon.refresh_tray();
            let quiet = lock(&daemon.state).quiet;
            match outcome {
                Ok(_) if quiet => {}
                Ok(Some(how)) => daemon.notify("✓ Dictation", how, 1500, false),
                Ok(None) => daemon.notify("No speech recognised", "", 2500, false),
                Err(error) => {
                    log(&format!("dictation failed: {error}"));
                    daemon.notify("Dictation failed", &error, 8000, true);
                }
            }
        });
        "ok transcribing".into()
    }

    fn finish(&self, recorder: Option<Recorder>) -> Result<Option<&'static str>, String> {
        let samples = recorder.ok_or("no active recording")?.finish()?;
        if samples.len() < SAMPLE_RATE / 4 {
            return Ok(None);
        }
        let config = Config::load();
        if !lock(&self.state).quiet {
            self.notify("… Transcribing", "", 600_000, false);
        }
        let started = Instant::now();
        let audio_seconds = samples.len() as f64 / SAMPLE_RATE as f64;
        // A panic anywhere in the engine stack is reported as an ordinary failure instead of
        // taking the service down; a lock poisoned by one is recovered by `lock`.
        let text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut slot = lock(&self.engine);
            match slot.as_mut() {
                Ok(engine) => engine.transcribe(&samples),
                Err(error) => Err(error.clone()),
            }
        }))
        .unwrap_or_else(|_| Err("transcription failed unexpectedly, please try again".into()))?;
        // Timing only: transcript content is never logged.
        log(&format!("transcribed {audio_seconds:.1}s of audio in {:.2}s", started.elapsed().as_secs_f64()));
        let text = text.trim().to_string();
        if text.is_empty() {
            return Ok(None);
        }
        let quiet = {
            let mut state = lock(&self.state);
            state.last_text = text.clone();
            state.quiet
        };
        if quiet {
            return Ok(Some("shown in the settings window"));
        }
        // The shortcut's modifier keys may still be down; typing into them would trigger
        // other shortcuts instead of inserting text.
        let waited = started.elapsed().as_millis() as u64;
        std::thread::sleep(Duration::from_millis(config.number("type_delay_ms").saturating_sub(waited)));
        let payload = if config.flag("append_space") { format!("{text} ") } else { text };
        crate::inject::deliver(&payload, &config.get("inject")).map(Some)
    }

    pub fn cancel(self: &Arc<Self>) -> String {
        let recorder = {
            let mut state = lock(&self.state);
            if state.phase != Phase::Recording {
                return "ok".into();
            }
            state.phase = Phase::Idle;
            state.session += 1;
            state.recorder.take()
        };
        drop(recorder);
        self.refresh_tray();
        self.notify("Recording discarded", "", 1500, false);
        "ok idle".into()
    }

    pub fn toggle(self: &Arc<Self>) -> String {
        let phase = lock(&self.state).phase;
        match phase {
            Phase::Recording => self.stop(),
            _ => self.start(),
        }
    }

    pub fn copy_last(&self) -> String {
        let text = lock(&self.state).last_text.clone();
        if text.is_empty() {
            return "error nothing transcribed yet".into();
        }
        match crate::inject::copy(&text) {
            Ok(()) => "ok copied".into(),
            Err(error) => format!("error {error}"),
        }
    }

    /// Transcribes an audio file with the loaded model, for the settings window.
    fn transcribe_file(&self, path: &str) -> String {
        let samples = match crate::audio_io::read_file(std::path::Path::new(path)) {
            Ok(samples) => samples,
            Err(error) => return format!("error {error}"),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut slot = lock(&self.engine);
            match slot.as_mut() {
                Ok(engine) => engine.transcribe(&samples),
                Err(error) => Err(error.clone()),
            }
        }))
        .unwrap_or_else(|_| Err("transcription failed unexpectedly".into()));
        match result {
            Ok(text) => format!("ok {}", text.replace('\n', " ")),
            Err(error) => format!("error {error}"),
        }
    }

    fn status(&self) -> String {
        let word = self.status_word();
        let state = lock(&self.state);
        let mut line = format!("{word} model={} backend={}", state.model, state.backend);
        if let Some(error) = &state.error {
            line.push_str(&format!(" error={error}"));
        }
        line
    }

    fn handle(self: &Arc<Self>, command: &str) -> String {
        match command {
            "toggle" => self.toggle(),
            "start" => self.start(),
            "stop" => self.stop(),
            "cancel" => self.cancel(),
            "start-test" => self.begin(true),
            "status" => self.status(),
            "languages" => format!("ok {}", lock(&self.state).languages.join(",")),
            "last" => format!("ok {}", lock(&self.state).last_text.replace('\n', " ")),
            "copy-last" => self.copy_last(),
            "reload" => {
                self.load_engine();
                "ok loading".into()
            }
            "quit" => "ok bye".into(),
            other => match other.strip_prefix("file ") {
                Some(path) => self.transcribe_file(path),
                None => format!("error unknown command '{other}'"),
            },
        }
    }
}

static SOCKET_PATH: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();

extern "C" fn on_signal(_signal: libc::c_int) {
    if let Some(path) = SOCKET_PATH.get() {
        unsafe { libc::unlink(path.as_ptr()) };
    }
    unsafe { libc::_exit(0) };
}

/// Sends one command to a running service.
pub fn request(command: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(crate::config::socket_path()).map_err(|e| e.to_string())?;
    stream.write_all(format!("{command}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    Ok(reply.trim().to_string())
}

pub fn run() -> Result<(), String> {
    let path = crate::config::socket_path();
    if request("status").is_ok() {
        return Err("ostt is already running".into());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| format!("cannot bind {}: {e}", path.display()))?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    let _ = SOCKET_PATH.set(std::ffi::CString::new(path.to_string_lossy().as_bytes()).unwrap());
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
    crate::inject::ensure_session_env();

    let daemon = Arc::new(Daemon {
        state: Mutex::new(State {
            phase: Phase::Idle,
            loading: true,
            error: None,
            recorder: None,
            session: 0,
            started: None,
            quiet: false,
            last_text: String::new(),
            model: String::new(),
            backend: String::new(),
            languages: Vec::new(),
        }),
        engine: Mutex::new(Err("model not loaded".into())),
        tray: Mutex::new(None),
    });
    match crate::tray::spawn(daemon.clone()) {
        Ok(handle) => *lock(&daemon.tray) = Some(handle),
        Err(error) => log(&format!("no tray icon ({error}); the shortcut still works")),
    }
    daemon.load_engine();
    log(&format!("listening on {}", path.display()));

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let daemon = daemon.clone();
        let path = path.clone();
        // One thread per request, so a long file transcription never blocks the shortcut.
        std::thread::spawn(move || {
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                return;
            }
            let command = line.trim();
            let reply = daemon.handle(command);
            let _ = (&stream).write_all(format!("{reply}\n").as_bytes());
            if command == "quit" {
                daemon.cancel();
                let _ = std::fs::remove_file(&path);
                std::process::exit(0);
            }
        });
    }
    Ok(())
}
